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
    const NUM_CHILDREN: usize = 3;
    try_stream! {
        // vec![ vec![] ]
        // ..
        // vec![ vec![0, 1, 2, 3, 4, 5, 6, 7] ]
        //   vec![ vec![], vec![] ]  ( [8, p0] , index = 0)
        //   vec![ vec![p0], vec![] ]  ( [8] , index = 1)
        // vec![ vec![p0], vec![8] ]

        // ...

        // vec![ vec![p0] vec![0, 1, 2, 3, 4, 5, 6, 7] ]
        // vec![ vec![p0, p1], vec![]]

        // ..

        // vec![ vec![p0, p1, p2, p3, p4, p5, p6, p7], [0, 1, 2, 3, 4, 5, 6, 7] ]
        //       ([8] index = 1)
        // vec![ vec![p0, p1, p2, p3, p4, p5, p6, p7], vec![] ] ([8, p8], index = 0)
        //
        // vec![ vec![], vec![], vec![] ] ([8, p8, pp0], index = 0)
        //
        // vec![ vec![pp0], vec![], vec![] ] ([8, p8], index = 0)
        //
        // vec![ vec![pp0], vec![p8], vec![8] ]

        let mut tree: VecDeque<Vec<usize>> = VecDeque::new();
        tree.push_back(Vec::with_capacity(NUM_CHILDREN));

        tokio::pin!(in_stream);

        while let Some(chunk) = in_stream.next().await {
            let mut index = tree.len() - 1;
            let mut next_nodes = vec![];
            loop {
                if tree[index].len() == NUM_CHILDREN {
                    let links = std::mem::replace(&mut tree[index], Vec::new());
                   let root_node = UnixfsNode::File(links);
                   next_nodes.push(root_node);

                   if index == 0 {
                        tree.push_front(vec![]);
                   } else {
                        index -= 1;
                   }
                } else {
                    if next_nodes.is_empty() {
                        break;
                    }
                    let next_node = next_nodes.pop().unwrap();
                    tree[index].push(next_node.cid());
                    yield next_node;
                }
            }

            index = tree.len() - 1;
            let raw = UnixfsNode::Raw(chunk);
            tree[index].push(raw.cid());
            yield raw;
        }

        // yield not filled subtrees
        while let Some(links) = tree.pop_back() {
            let node = UnixfsNode::File(links);
            let cid = node.cid();
            yield node;

            if let Some(last) = tree.back_mut() {
                last.push(cid);
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
