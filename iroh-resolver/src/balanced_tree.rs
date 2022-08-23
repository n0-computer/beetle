use std::collections::VecDeque;

use anyhow::Result;
use async_stream::try_stream;
use bytes::{Bytes, BytesMut};
use cid::Cid;
use futures::{Stream, StreamExt};

use crate::unixfs::{dag_pb, unixfs_pb, DataType, Node, UnixfsNode};
use crate::unixfs_builder::encode_unixfs_pb;

// 3                          root
// 2                                 nextparent0
// 1             parent0              parent1    ... parent7
// 0 [0, 1, 2, 3, 4, 5, 6, 7]      [0, ... ]
//

pub fn stream_balanced_tree(
    in_stream: impl Stream<Item = std::io::Result<BytesMut>>,
    max_degrees: usize,
) -> impl Stream<Item = Result<(Cid, Bytes)>> {
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

        let mut tree: VecDeque<Vec<(Cid, u64)>> = VecDeque::new();
        tree.push_back(Vec::with_capacity(max_degrees));

        tokio::pin!(in_stream);

        while let Some(chunk) = in_stream.next().await {
            let chunk = chunk?;
            let chunk = chunk.freeze();
            let tree_len = tree.len();

            // check if the leaf node of the tree is full
            if tree[0].len() == max_degrees {
                // if so, iterater through nodes
                for i in 0..tree_len {
                    // if we encounter any nodes that are not full, break
                    if tree[i].len() < max_degrees {
                        break;
                    }

                    // in this case we have a full set of links & we are
                    // at the top of the tree. Time to make a new layer.
                    if i == tree_len - 1 {
                        tree.push_back(vec![]);
                    }

                    // create node, keeping the cid
                    let links = std::mem::replace(&mut tree[i], Vec::new());
                    let (cid, bytes, len) = TreeNode::Stem(links).encode()?;
                    yield (cid, bytes);

                    // add cid to parent node
                    tree[i+1].push((cid, len));
                }
                // at this point the tree will be able to recieve new links
                // without "overflowing", aka the leaf node and stem nodes
                // have fewer than max_degrees number of links
            }

            // now that we know the tree is in a "healthy" state to
            // recieve more links, add the link to the tree
            let (cid, bytes, len) = TreeNode::Leaf(chunk).encode()?;
            tree[0].push((cid, len));
            yield (cid, bytes);
            // at this point, the leaf node may have max_degrees number of
            // links, but no other stem node will
        }

        // our stream had 1 chunk that we have already yielded
        if tree.len() == 1 && tree[0].len() == 1 {
            return
        }

        // clean up, aka yield the rest of the stem nodes
        // since all the stem nodes are able to recieve links
        // we don't have to worry about "overflow"
        while let Some(links) = tree.pop_front() {
            // root node
            // if tree.len() == 0 {
            //     let (cid, bytes) = TreeNode::Root(links).encode()?;
            //     yield (cid, bytes);
            //     return
            // }

            let (cid, bytes, len) = TreeNode::Stem(links).encode()?;
            yield (cid, bytes);

            if let Some(front) = tree.front_mut() {
                front.push((cid, len));
            } else {
                // final root, nothing to do
            }
        }
    }
}

fn create_unixfs_node_from_links(links: Vec<(Cid, u64)>, is_root: bool) -> Result<UnixfsNode> {
    let links = links
        .into_iter()
        .map(|(cid, len)| dag_pb::PbLink {
            hash: Some(cid.to_bytes()),
            // TODO: notes say that leaf chunks have "", does that mean that
            // stem nodes should have None?
            name: Some("".into()),
            tsize: Some(len as u64),
        })
        .collect();

    // PBNode.Data
    let inner = unixfs_pb::Data {
        r#type: DataType::File as i32,
        ..Default::default()
    };

    // create PBNode
    let outer = encode_unixfs_pb(&inner, links)?;

    if is_root {
        return Ok(UnixfsNode::Directory(Node { inner, outer }));
    }

    // create UnixfsNode
    Ok(UnixfsNode::File(Node { inner, outer }))
}

// Leaf and Stem nodes are the two types of nodes that can exist in the tree
// Leaf nodes encode to `UnixfsNode::Raw`
// Stem nodes encode to `UnixfsNode::File`
// Root nodes encode to `UnixfsNode::Directory`
enum TreeNode {
    Leaf(Bytes),
    Stem(Vec<(Cid, u64)>),
    // Root(Vec<(Cid, u64)>),
}

impl TreeNode {
    fn encode(self) -> Result<(Cid, Bytes, u64)> {
        match self {
            TreeNode::Leaf(bytes) => {
                let node = UnixfsNode::Raw(bytes);
                let (cid, bytes) = node.encode()?;
                let len = bytes.len();
                Ok((cid, bytes, len as u64))
            }
            TreeNode::Stem(links) => {
                // keep track of `tsize`, aka the size of the encoded tree at the given link
                let mut cumulative_len: u64 = links.iter().map(|(_, len)| len).sum();
                let node = create_unixfs_node_from_links(links, false)?;
                let (cid, bytes) = node.encode()?;
                cumulative_len += bytes.len() as u64;
                Ok((cid, bytes, cumulative_len))
            } // TreeNode::Root(cids) => {
              //     let cumulative_len: u64 = links.iter().map(|(_, len)| len).sum();
              //     let node = create_unixfs_node_from_links(cids, true)?;
              //     let (cid, bytes) = node.encode()?;
              //     cumulative_len += bytes.len() as u64;
              //     Ok((cid, bytes, cumulative_len))
              // }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_chunk_stream(num_chunks: usize) -> impl Stream<Item = std::io::Result<BytesMut>> {
        async_stream::try_stream! {
            for n in 0..num_chunks {
                let bytes = BytesMut::from(&n.to_be_bytes()[..]);
                yield bytes
            }
        }
    }

    async fn ensure_equal(
        expect: Vec<(Cid, Bytes)>,
        got: impl Stream<Item = Result<(Cid, Bytes)>>,
    ) {
        let mut i = 0;
        tokio::pin!(got);
        while let Some(node) = got.next().await {
            let (expect_cid, expect_bytes) = expect
                .get(i)
                .expect("too many nodes in balanced tree stream");
            let node = node.expect("unexpected error in balanced tree stream");
            let (got_cid, got_bytes) = node;
            println!("node index {}", i);
            assert_eq!(*expect_cid, got_cid);
            assert_eq!(*expect_bytes, got_bytes);
            i += 1;
        }
        if expect.len() != i {
            panic!(
                "expected at {} nodes of the stream, got {}",
                expect.len(),
                i
            );
        }
    }

    #[tokio::test]
    async fn balanced_tree_test_leaf() {
        let chunk_stream = test_chunk_stream(1);
        tokio::pin!(chunk_stream);

        // "manually" build expected stream output
        let mut expect = Vec::new();
        let chunk = chunk_stream.next().await.unwrap().unwrap().freeze();
        let node = TreeNode::Leaf(chunk.clone());
        let (cid, bytes, _) = node.encode().unwrap();
        expect.push((cid, bytes.clone()));
        let got = stream_balanced_tree(test_chunk_stream(1), 3);
        tokio::pin!(got);
        ensure_equal(expect, got).await;
    }

    #[tokio::test]
    async fn balanced_tree_test_height_one() {
        let num_chunks = 3;
        let degrees = 3;
        let chunk_stream = test_chunk_stream(num_chunks);
        tokio::pin!(chunk_stream);

        // "manually" build expected stream output
        let mut expect = Vec::new();
        let mut links = Vec::new();
        while let Some(chunk) = chunk_stream.next().await {
            let chunk = chunk.unwrap().freeze();
            let node = TreeNode::Leaf(chunk);
            let (cid, bytes, len) = node.encode().unwrap();
            links.push((cid, len));
            expect.push((cid, bytes));
        }
        // let root = TreeNode::Root(links);
        let root = TreeNode::Stem(links);
        let (cid, bytes, _) = root.encode().unwrap();
        expect.push((cid, bytes));

        let got = stream_balanced_tree(test_chunk_stream(num_chunks), degrees);
        tokio::pin!(got);
        ensure_equal(expect, got).await;
    }

    #[tokio::test]
    async fn balanced_tree_test_height_two_full() {
        let degrees = 3;
        let num_chunks = 9;
        let chunk_stream = test_chunk_stream(num_chunks);
        tokio::pin!(chunk_stream);

        // "manually" build expected stream output
        let mut expect = Vec::new();
        let mut links_one = Vec::new();
        let mut links_two = Vec::new();
        while let Some(chunk) = chunk_stream.next().await {
            let chunk = chunk.unwrap().freeze();
            let node = TreeNode::Leaf(chunk);
            let (cid, bytes, len) = node.encode().unwrap();
            links_one.push((cid, len));
            expect.push((cid, bytes));
            if links_one.len() == degrees {
                let links = std::mem::take(&mut links_one);
                let stem = TreeNode::Stem(links);
                let (cid, bytes, len) = stem.encode().unwrap();
                links_two.push((cid, len));
                expect.push((cid, bytes));
            }
        }
        // let root = TreeNode::Root(links_two);
        let root = TreeNode::Stem(links_two);
        let (cid, bytes, _) = root.encode().unwrap();
        expect.push((cid, bytes));

        let got = stream_balanced_tree(test_chunk_stream(num_chunks), degrees);
        tokio::pin!(got);
        ensure_equal(expect, got).await;
    }

    #[tokio::test]
    async fn balanced_tree_test_height_two_not_full() {
        let num_chunks = 7;
        let degrees = 3;
        let chunk_stream = test_chunk_stream(num_chunks);
        tokio::pin!(chunk_stream);

        // "manually" build expected stream output
        let mut expect = Vec::new();
        let mut links_one = Vec::new();
        let mut links_two = Vec::new();
        while let Some(chunk) = chunk_stream.next().await {
            let chunk = chunk.unwrap().freeze();
            let node = TreeNode::Leaf(chunk);
            let (cid, bytes, len) = node.encode().unwrap();
            links_one.push((cid, len));
            expect.push((cid, bytes));
            if links_one.len() == degrees {
                let links = std::mem::take(&mut links_one);
                let stem = TreeNode::Stem(links);
                let (cid, bytes, len) = stem.encode().unwrap();
                links_two.push((cid, len));
                expect.push((cid, bytes));
            }
        }
        let stem = TreeNode::Stem(links_one);
        let (cid, bytes, len) = stem.encode().unwrap();
        links_two.push((cid, len));
        expect.push((cid, bytes));
        // let root = TreeNode::Root(links_two);
        let root = TreeNode::Stem(links_two);
        let (cid, bytes, _) = root.encode().unwrap();
        expect.push((cid, bytes));

        let got = stream_balanced_tree(test_chunk_stream(num_chunks), degrees);
        tokio::pin!(got);
        ensure_equal(expect, got).await;
    }

    #[tokio::test]
    async fn balanced_tree_test_height_three() {
        let num_chunks = 17;
        let degrees = 3;
        let chunk_stream = test_chunk_stream(num_chunks);
        tokio::pin!(chunk_stream);

        // "manually" build expected stream output
        let mut expect = Vec::new();
        let mut links_one = Vec::new();
        let mut links_two = Vec::new();
        let mut links_three = Vec::new();
        while let Some(chunk) = chunk_stream.next().await {
            let chunk = chunk.unwrap().freeze();
            let node = TreeNode::Leaf(chunk);
            let (cid, bytes, len) = node.encode().unwrap();
            links_one.push((cid, len));
            expect.push((cid, bytes));
            if links_one.len() == degrees {
                let links = std::mem::take(&mut links_one);
                let stem = TreeNode::Stem(links);
                let (cid, bytes, len) = stem.encode().unwrap();
                links_two.push((cid, len));
                expect.push((cid, bytes));
                if links_two.len() == degrees {
                    let links = std::mem::take(&mut links_two);
                    let stem = TreeNode::Stem(links);
                    let (cid, bytes, len) = stem.encode().unwrap();
                    links_three.push((cid, len));
                    expect.push((cid, bytes));
                }
            }
        }
        let one = TreeNode::Stem(links_one);
        let (cid, bytes, len) = one.encode().unwrap();
        links_two.push((cid, len));
        expect.push((cid, bytes));

        let two = TreeNode::Stem(links_two);
        let (cid, bytes, len) = two.encode().unwrap();
        links_three.push((cid, len));
        expect.push((cid, bytes));

        // let root = TreeNode::Root(links_three);
        let root = TreeNode::Stem(links_three);
        let (cid, bytes, _) = root.encode().unwrap();
        expect.push((cid, bytes));

        let got = stream_balanced_tree(test_chunk_stream(num_chunks), degrees);
        tokio::pin!(got);
        ensure_equal(expect, got).await;
    }
}
