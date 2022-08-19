use std::collections::VecDeque;

use anyhow::Result;
use async_stream::try_stream;
use futures::{Stream, StreamExt};

pub enum UnixfsNode {
    Raw(usize),
    File(Vec<usize>),
}

impl UnixfsNode {
    fn cid(&self) -> usize {
        match self {
            UnixfsNode::Raw(cid) => *cid,
            UnixfsNode::File(cids) => cids.iter().sum(),
        }
    }
}

impl std::fmt::Display for UnixfsNode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UnixfsNode::Raw(cid) => write!(f, "{}", cid),
            UnixfsNode::File(cids) => {
                write!(f, "{:?} - {}", cids, self.cid())
            }
        }
    }
}

// 3                          root
// 2                                 nextparent0
// 1             parent0              parent1    ... parent7
// 0 [0, 1, 2, 3, 4, 5, 6, 7]      [0, ... ]

pub fn build_stream(
    in_stream: impl Stream<Item = usize>,
) -> impl Stream<Item = Result<UnixfsNode>> {
    const MAX_DEGREES: usize = 3;
    try_stream! {
        // vec![ vec![] ]
        // ..
        // vec![ vec![0, 1, 2, 3, 4, 5, 6, 7] ]
        // vec![ vec![8], vec![p0] ]

        // ...

        // vec![ vec![0, 1, 2, 3, 4, 5, 6, 7] vec![p0] ]
        // vec![ vec![], vec![p0, p1]]

        // ..

        // vec![ vec![0, 1, 2, 3, 4, 5, 6, 7] vec![p0, p1, p2, p3, p4, p5, p6, p7], ]
        // vec![ vec![], vec![p0, p1, p2, p3, p4, p5, p6, p7], vec![] ]
        // vec![ vec![8], vec![p8], vec![pp0] ]

        let mut tree: VecDeque<Vec<usize>> = VecDeque::new();
        tree.push_back(Vec::with_capacity(MAX_DEGREES));

        tokio::pin!(in_stream);

        while let Some(chunk) = in_stream.next().await {
            let tree_len = tree.len();

            // check if the leaf node of the tree is full
            if tree[0].len() == MAX_DEGREES {
                // if so, iterater through nodes
                for i in 0..tree_len {
                    // if we encounter any nodes that are not full, break
                    if tree[i].len() < MAX_DEGREES {
                        break;
                    }

                    // in this case we have a full set of links & we are
                    // at the top of the tree. Time to make a new layer.
                    if i == tree_len - 1 {
                        tree.push_back(vec![]);
                    }

                    // create node, keeping the cid
                    let links = std::mem::replace(&mut tree[i], Vec::new());
                    let node = UnixfsNode::File(links);
                    let cid = node.cid();
                    yield node;

                    // add cid to parent node
                    tree[i+1].push(cid);
                }
            }

            // now that we know the tree is in a "healthy" state to
            // recieve more links, add the link to the tree
            let raw = UnixfsNode::Raw(chunk);
            tree[0].push(raw.cid());
            yield raw;
        }

        // yield not filled subtrees
        while let Some(links) = tree.pop_front() {
            let node = UnixfsNode::File(links);
            let cid = node.cid();
            yield node;

            if let Some(front) = tree.front_mut() {
                front.push(cid);
            } else {
                // final root, nothing to do
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn print_test() {
        let len = 78;
        let v: Vec<usize> = (1..=len).collect();
        let stream = futures::stream::iter(v);
        let unixfs_node_stream = build_stream(stream);
        tokio::pin!(unixfs_node_stream);
        while let Some(node) = unixfs_node_stream.next().await {
            println!("{}", node.unwrap());
        }
    }
}
