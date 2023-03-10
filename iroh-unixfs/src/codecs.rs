/// Multicodecs, as defined in <https://github.com/multiformats/multicodec/blob/master/table.csv>.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive, Hash,
)]
#[repr(u64)]
pub enum Codec {
    Identity = 0x00,
    Cidv1 = 0x01,
    Cidv2 = 0x02,
    Cidv3 = 0x03,
    Ip4 = 0x04,
    Tcp = 0x06,
    Sha1 = 0x11,
    Sha2256 = 0x12,
    Sha2512 = 0x13,
    Sha3512 = 0x14,
    Sha3384 = 0x15,
    Sha3256 = 0x16,
    Sha3224 = 0x17,
    Shake128 = 0x18,
    Shake256 = 0x19,
    Keccak224 = 0x1a,
    Keccak256 = 0x1b,
    Keccak384 = 0x1c,
    Keccak512 = 0x1d,
    Blake3 = 0x1e,
    Sha2384 = 0x20,
    Dccp = 0x21,
    Murmur3X6464 = 0x22,
    Murmur332 = 0x23,
    Ip6 = 0x29,
    Ip6zone = 0x2a,
    Path = 0x2f,
    Multicodec = 0x30,
    Multihash = 0x31,
    Multiaddr = 0x32,
    Multibase = 0x33,
    Dns = 0x35,
    Dns4 = 0x36,
    Dns6 = 0x37,
    Dnsaddr = 0x38,
    Protobuf = 0x50,
    Cbor = 0x51,
    Raw = 0x55,
    DblSha2256 = 0x56,
    Rlp = 0x60,
    Bencode = 0x63,
    DagPb = 0x70,
    DagCbor = 0x71,
    Libp2pKey = 0x72,
    GitRaw = 0x78,
    TorrentInfo = 0x7b,
    TorrentFile = 0x7c,
    LeofcoinBlock = 0x81,
    LeofcoinTx = 0x82,
    LeofcoinPr = 0x83,
    Sctp = 0x84,
    DagJose = 0x85,
    DagCose = 0x86,
    EthBlock = 0x90,
    EthBlockList = 0x91,
    EthTxTrie = 0x92,
    EthTx = 0x93,
    EthTxReceiptTrie = 0x94,
    EthTxReceipt = 0x95,
    EthStateTrie = 0x96,
    EthAccountSnapshot = 0x97,
    EthStorageTrie = 0x98,
    EthReceiptLogTrie = 0x99,
    EthRecieptLog = 0x9a,
    Aes128 = 0xa0,
    Aes192 = 0xa1,
    Aes256 = 0xa2,
    Chacha128 = 0xa3,
    Chacha256 = 0xa4,
    BitcoinBlock = 0xb0,
    BitcoinTx = 0xb1,
    BitcoinWitnessCommitment = 0xb2,
    ZcashBlock = 0xc0,
    ZcashTx = 0xc1,
    Caip50 = 0xca,
    Streamid = 0xce,
    StellarBlock = 0xd0,
    StellarTx = 0xd1,
    Md4 = 0xd4,
    Md5 = 0xd5,
    Bmt = 0xd6,
    DecredBlock = 0xe0,
    DecredTx = 0xe1,
    IpldNs = 0xe2,
    IpfsNs = 0xe3,
    SwarmNs = 0xe4,
    IpnsNs = 0xe5,
    Zeronet = 0xe6,
    Secp256k1Pub = 0xe7,
    Bls12_381G1Pub = 0xea,
    Bls12_381G2Pub = 0xeb,
    X25519Pub = 0xec,
    Ed25519Pub = 0xed,
    Bls12_381G1g2Pub = 0xee,
    DashBlock = 0xf0,
    DashTx = 0xf1,
    SwarmManifest = 0xfa,
    SwarmFeed = 0xfb,
    Udp = 0x0111,
    P2pWebrtcStar = 0x0113,
    P2pWebrtcDirect = 0x0114,
    P2pStardust = 0x0115,
    P2pCircuit = 0x0122,
    DagJson = 0x0129,
    Udt = 0x012d,
    Utp = 0x012e,
    Unix = 0x0190,
    Thread = 0x0196,
    P2p = 0x01a5,
    Https = 0x01bb,
    Onion = 0x01bc,
    Onion3 = 0x01bd,
    Garlic64 = 0x01be,
    Garlic32 = 0x01bf,
    Tls = 0x01c0,
    Noise = 0x01c6,
    Quic = 0x01cc,
    Webtransport = 0x01d1,
    Ws = 0x01dd,
    Wss = 0x01de,
    P2pWebsocketStar = 0x01df,
    Http = 0x01e0,
    Swhid1Snp = 0x01f0,
    Json = 0x0200,
    Messagepack = 0x0201,
    Car = 0x0202,
    Libp2pPeerRecord = 0x0301,
    Libp2pRelayRsvp = 0x0302,
    CarIndexSorted = 0x0400,
    CarMultihashIndexSorted = 0x0401,
    TransportBitswap = 0x0900,
    TransportGraphsyncFilecoinv1 = 0x0910,
    Sha2256Trunc254Padded = 0x1012,
    Sha2224 = 0x1013,
    Sha2512224 = 0x1014,
    Sha2512256 = 0x1015,
    Murmur3X64128 = 0x1022,
    Ripemd128 = 0x1052,
    Ripemd160 = 0x1053,
    Ripemd256 = 0x1054,
    Ripemd320 = 0x1055,
    X11 = 0x1100,
    P256Pub = 0x1200,
    P384Pub = 0x1201,
    P521Pub = 0x1202,
    Ed448Pub = 0x1203,
    X448Pub = 0x1204,
    RsaPub = 0x1205,
    Ed25519Priv = 0x1300,
    Secp256k1Priv = 0x1301,
    X25519Priv = 0x1302,
    Kangarootwelve = 0x1d01,
    Sm3256 = 0x534d,
    Blake2b8 = 0xb201,
    Blake2b16 = 0xb202,
    Blake2b24 = 0xb203,
    Blake2b32 = 0xb204,
    Blake2b40 = 0xb205,
    Blake2b48 = 0xb206,
    Blake2b56 = 0xb207,
    Blake2b64 = 0xb208,
    Blake2b72 = 0xb209,
    Blake2b80 = 0xb20a,
    Blake2b88 = 0xb20b,
    Blake2b96 = 0xb20c,
    Blake2b104 = 0xb20d,
    Blake2b112 = 0xb20e,
    Blake2b120 = 0xb20f,
    Blake2b128 = 0xb210,
    Blake2b136 = 0xb211,
    Blake2b144 = 0xb212,
    Blake2b152 = 0xb213,
    Blake2b160 = 0xb214,
    Blake2b168 = 0xb215,
    Blake2b176 = 0xb216,
    Blake2b184 = 0xb217,
    Blake2b192 = 0xb218,
    Blake2b200 = 0xb219,
    Blake2b208 = 0xb21a,
    Blake2b216 = 0xb21b,
    Blake2b224 = 0xb21c,
    Blake2b232 = 0xb21d,
    Blake2b240 = 0xb21e,
    Blake2b248 = 0xb21f,
    Blake2b256 = 0xb220,
    Blake2b264 = 0xb221,
    Blake2b272 = 0xb222,
    Blake2b280 = 0xb223,
    Blake2b288 = 0xb224,
    Blake2b296 = 0xb225,
    Blake2b304 = 0xb226,
    Blake2b312 = 0xb227,
    Blake2b320 = 0xb228,
    Blake2b328 = 0xb229,
    Blake2b336 = 0xb22a,
    Blake2b344 = 0xb22b,
    Blake2b352 = 0xb22c,
    Blake2b360 = 0xb22d,
    Blake2b368 = 0xb22e,
    Blake2b376 = 0xb22f,
    Blake2b384 = 0xb230,
    Blake2b392 = 0xb231,
    Blake2b400 = 0xb232,
    Blake2b408 = 0xb233,
    Blake2b416 = 0xb234,
    Blake2b424 = 0xb235,
    Blake2b432 = 0xb236,
    Blake2b440 = 0xb237,
    Blake2b448 = 0xb238,
    Blake2b456 = 0xb239,
    Blake2b464 = 0xb23a,
    Blake2b472 = 0xb23b,
    Blake2b480 = 0xb23c,
    Blake2b488 = 0xb23d,
    Blake2b496 = 0xb23e,
    Blake2b504 = 0xb23f,
    Blake2b512 = 0xb240,
    Blake2s8 = 0xb241,
    Blake2s16 = 0xb242,
    Blake2s24 = 0xb243,
    Blake2s32 = 0xb244,
    Blake2s40 = 0xb245,
    Blake2s48 = 0xb246,
    Blake2s56 = 0xb247,
    Blake2s64 = 0xb248,
    Blake2s72 = 0xb249,
    Blake2s80 = 0xb24a,
    Blake2s88 = 0xb24b,
    Blake2s96 = 0xb24c,
    Blake2s104 = 0xb24d,
    Blake2s112 = 0xb24e,
    Blake2s120 = 0xb24f,
    Blake2s128 = 0xb250,
    Blake2s136 = 0xb251,
    Blake2s144 = 0xb252,
    Blake2s152 = 0xb253,
    Blake2s160 = 0xb254,
    Blake2s168 = 0xb255,
    Blake2s176 = 0xb256,
    Blake2s184 = 0xb257,
    Blake2s192 = 0xb258,
    Blake2s200 = 0xb259,
    Blake2s208 = 0xb25a,
    Blake2s216 = 0xb25b,
    Blake2s224 = 0xb25c,
    Blake2s232 = 0xb25d,
    Blake2s240 = 0xb25e,
    Blake2s248 = 0xb25f,
    Blake2s256 = 0xb260,
    Skein2568 = 0xb301,
    Skein25616 = 0xb302,
    Skein25624 = 0xb303,
    Skein25632 = 0xb304,
    Skein25640 = 0xb305,
    Skein25648 = 0xb306,
    Skein25656 = 0xb307,
    Skein25664 = 0xb308,
    Skein25672 = 0xb309,
    Skein25680 = 0xb30a,
    Skein25688 = 0xb30b,
    Skein25696 = 0xb30c,
    Skein256104 = 0xb30d,
    Skein256112 = 0xb30e,
    Skein256120 = 0xb30f,
    Skein256128 = 0xb310,
    Skein256136 = 0xb311,
    Skein256144 = 0xb312,
    Skein256152 = 0xb313,
    Skein256160 = 0xb314,
    Skein256168 = 0xb315,
    Skein256176 = 0xb316,
    Skein256184 = 0xb317,
    Skein256192 = 0xb318,
    Skein256200 = 0xb319,
    Skein256208 = 0xb31a,
    Skein256216 = 0xb31b,
    Skein256224 = 0xb31c,
    Skein256232 = 0xb31d,
    Skein256240 = 0xb31e,
    Skein256248 = 0xb31f,
    Skein256256 = 0xb320,
    Skein5128 = 0xb321,
    Skein51216 = 0xb322,
    Skein51224 = 0xb323,
    Skein51232 = 0xb324,
    Skein51240 = 0xb325,
    Skein51248 = 0xb326,
    Skein51256 = 0xb327,
    Skein51264 = 0xb328,
    Skein51272 = 0xb329,
    Skein51280 = 0xb32a,
    Skein51288 = 0xb32b,
    Skein51296 = 0xb32c,
    Skein512104 = 0xb32d,
    Skein512112 = 0xb32e,
    Skein512120 = 0xb32f,
    Skein512128 = 0xb330,
    Skein512136 = 0xb331,
    Skein512144 = 0xb332,
    Skein512152 = 0xb333,
    Skein512160 = 0xb334,
    Skein512168 = 0xb335,
    Skein512176 = 0xb336,
    Skein512184 = 0xb337,
    Skein512192 = 0xb338,
    Skein512200 = 0xb339,
    Skein512208 = 0xb33a,
    Skein512216 = 0xb33b,
    Skein512224 = 0xb33c,
    Skein512232 = 0xb33d,
    Skein512240 = 0xb33e,
    Skein512248 = 0xb33f,
    Skein512256 = 0xb340,
    Skein512264 = 0xb341,
    Skein512272 = 0xb342,
    Skein512280 = 0xb343,
    Skein512288 = 0xb344,
    Skein512296 = 0xb345,
    Skein512304 = 0xb346,
    Skein512312 = 0xb347,
    Skein512320 = 0xb348,
    Skein512328 = 0xb349,
    Skein512336 = 0xb34a,
    Skein512344 = 0xb34b,
    Skein512352 = 0xb34c,
    Skein512360 = 0xb34d,
    Skein512368 = 0xb34e,
    Skein512376 = 0xb34f,
    Skein512384 = 0xb350,
    Skein512392 = 0xb351,
    Skein512400 = 0xb352,
    Skein512408 = 0xb353,
    Skein512416 = 0xb354,
    Skein512424 = 0xb355,
    Skein512432 = 0xb356,
    Skein512440 = 0xb357,
    Skein512448 = 0xb358,
    Skein512456 = 0xb359,
    Skein512464 = 0xb35a,
    Skein512472 = 0xb35b,
    Skein512480 = 0xb35c,
    Skein512488 = 0xb35d,
    Skein512496 = 0xb35e,
    Skein512504 = 0xb35f,
    Skein512512 = 0xb360,
    Skein10248 = 0xb361,
    Skein102416 = 0xb362,
    Skein102424 = 0xb363,
    Skein102432 = 0xb364,
    Skein102440 = 0xb365,
    Skein102448 = 0xb366,
    Skein102456 = 0xb367,
    Skein102464 = 0xb368,
    Skein102472 = 0xb369,
    Skein102480 = 0xb36a,
    Skein102488 = 0xb36b,
    Skein102496 = 0xb36c,
    Skein1024104 = 0xb36d,
    Skein1024112 = 0xb36e,
    Skein1024120 = 0xb36f,
    Skein1024128 = 0xb370,
    Skein1024136 = 0xb371,
    Skein1024144 = 0xb372,
    Skein1024152 = 0xb373,
    Skein1024160 = 0xb374,
    Skein1024168 = 0xb375,
    Skein1024176 = 0xb376,
    Skein1024184 = 0xb377,
    Skein1024192 = 0xb378,
    Skein1024200 = 0xb379,
    Skein1024208 = 0xb37a,
    Skein1024216 = 0xb37b,
    Skein1024224 = 0xb37c,
    Skein1024232 = 0xb37d,
    Skein1024240 = 0xb37e,
    Skein1024248 = 0xb37f,
    Skein1024256 = 0xb380,
    Skein1024264 = 0xb381,
    Skein1024272 = 0xb382,
    Skein1024280 = 0xb383,
    Skein1024288 = 0xb384,
    Skein1024296 = 0xb385,
    Skein1024304 = 0xb386,
    Skein1024312 = 0xb387,
    Skein1024320 = 0xb388,
    Skein1024328 = 0xb389,
    Skein1024336 = 0xb38a,
    Skein1024344 = 0xb38b,
    Skein1024352 = 0xb38c,
    Skein1024360 = 0xb38d,
    Skein1024368 = 0xb38e,
    Skein1024376 = 0xb38f,
    Skein1024384 = 0xb390,
    Skein1024392 = 0xb391,
    Skein1024400 = 0xb392,
    Skein1024408 = 0xb393,
    Skein1024416 = 0xb394,
    Skein1024424 = 0xb395,
    Skein1024432 = 0xb396,
    Skein1024440 = 0xb397,
    Skein1024448 = 0xb398,
    Skein1024456 = 0xb399,
    Skein1024464 = 0xb39a,
    Skein1024472 = 0xb39b,
    Skein1024480 = 0xb39c,
    Skein1024488 = 0xb39d,
    Skein1024496 = 0xb39e,
    Skein1024504 = 0xb39f,
    Skein1024512 = 0xb3a0,
    Skein1024520 = 0xb3a1,
    Skein1024528 = 0xb3a2,
    Skein1024536 = 0xb3a3,
    Skein1024544 = 0xb3a4,
    Skein1024552 = 0xb3a5,
    Skein1024560 = 0xb3a6,
    Skein1024568 = 0xb3a7,
    Skein1024576 = 0xb3a8,
    Skein1024584 = 0xb3a9,
    Skein1024592 = 0xb3aa,
    Skein1024600 = 0xb3ab,
    Skein1024608 = 0xb3ac,
    Skein1024616 = 0xb3ad,
    Skein1024624 = 0xb3ae,
    Skein1024632 = 0xb3af,
    Skein1024640 = 0xb3b0,
    Skein1024648 = 0xb3b1,
    Skein1024656 = 0xb3b2,
    Skein1024664 = 0xb3b3,
    Skein1024672 = 0xb3b4,
    Skein1024680 = 0xb3b5,
    Skein1024688 = 0xb3b6,
    Skein1024696 = 0xb3b7,
    Skein1024704 = 0xb3b8,
    Skein1024712 = 0xb3b9,
    Skein1024720 = 0xb3ba,
    Skein1024728 = 0xb3bb,
    Skein1024736 = 0xb3bc,
    Skein1024744 = 0xb3bd,
    Skein1024752 = 0xb3be,
    Skein1024760 = 0xb3bf,
    Skein1024768 = 0xb3c0,
    Skein1024776 = 0xb3c1,
    Skein1024784 = 0xb3c2,
    Skein1024792 = 0xb3c3,
    Skein1024800 = 0xb3c4,
    Skein1024808 = 0xb3c5,
    Skein1024816 = 0xb3c6,
    Skein1024824 = 0xb3c7,
    Skein1024832 = 0xb3c8,
    Skein1024840 = 0xb3c9,
    Skein1024848 = 0xb3ca,
    Skein1024856 = 0xb3cb,
    Skein1024864 = 0xb3cc,
    Skein1024872 = 0xb3cd,
    Skein1024880 = 0xb3ce,
    Skein1024888 = 0xb3cf,
    Skein1024896 = 0xb3d0,
    Skein1024904 = 0xb3d1,
    Skein1024912 = 0xb3d2,
    Skein1024920 = 0xb3d3,
    Skein1024928 = 0xb3d4,
    Skein1024936 = 0xb3d5,
    Skein1024944 = 0xb3d6,
    Skein1024952 = 0xb3d7,
    Skein1024960 = 0xb3d8,
    Skein1024968 = 0xb3d9,
    Skein1024976 = 0xb3da,
    Skein1024984 = 0xb3db,
    Skein1024992 = 0xb3dc,
    Skein10241000 = 0xb3dd,
    Skein10241008 = 0xb3de,
    Skein10241016 = 0xb3df,
    Skein10241024 = 0xb3e0,
    PoseidonBls12_381A2Fc1 = 0xb401,
    PoseidonBls12_381A2Fc1Sc = 0xb402,
    Iscc = 0xcc01,
    ZeroxcertImprint256 = 0xce11,
    FilCommitmentUnsealed = 0xf101,
    FilCommitmentSealed = 0xf102,
    Plaintextv2 = 0x706c61,
    HolochainAdrV0 = 0x807124,
    HolochainAdrV1 = 0x817124,
    HolochainKeyV0 = 0x947124,
    HolochainKeyV1 = 0x957124,
    HolochainSigV0 = 0xa27124,
    HolochainSigV1 = 0xa37124,
    SkynetNs = 0xb19910,
    ArweaveNs = 0xb29910,
    SubspaceNs = 0xb39910,
    KumandraNs = 0xb49910,
}
