import {
  Keypair,
  clusterApiUrl,
  PublicKey as Web3JsPublicKey,
  LAMPORTS_PER_SOL,
  SystemProgram,
  Transaction,
  sendAndConfirmTransaction,
  Connection,
  TransactionSignature,
} from "@solana/web3.js";
import { createMint, getAssociatedTokenAddress } from "@solana/spl-token";
import * as anchor from "@project-serum/anchor";
import { NftStakingReal } from "../target/types/nft_staking_real";
import {
  fromWeb3JsPublicKey,
  toWeb3JsPublicKey,
} from "@metaplex-foundation/umi-web3js-adapters";

import { Program } from "@coral-xyz/anchor";
import { createUmi } from "@metaplex-foundation/umi-bundle-defaults";
import { bundlrUploader } from "@metaplex-foundation/umi-uploader-bundlr";
import {
  PublicKey,
  createSignerFromKeypair,
  generateSigner,
  lamports,
  percentAmount,
  signerIdentity,
} from "@metaplex-foundation/umi";
import {
  createNft,
  mplTokenMetadata,
  fetchDigitalAsset,
} from "@metaplex-foundation/mpl-token-metadata";

const NFT_URI =
  "https://arweave.net/MDYtr-vyVoLb4Xi4VA5httrOKOLCHeilWIxO6R00ex4";

export interface NftMint {
  tokenAccount: Web3JsPublicKey;
  mintAddress: Web3JsPublicKey;
  masterEditionAddress: Web3JsPublicKey;
}

/**
 * Wrapper for transaction handler to confirm transaction
 * @param connection Connection
 * @param transaction Transaction Promise
 */
const safeTransactionHandler = async (
  connection: Connection,
  transaction: Promise<TransactionSignature>
) => {
  const latestBlockhash = await connection.getLatestBlockhash();
  const txSig = await transaction;
  await connection.confirmTransaction({
    blockhash: latestBlockhash.blockhash,
    lastValidBlockHeight: latestBlockhash.lastValidBlockHeight,
    signature: txSig,
  });
};

export const setupNft = async (
  program: Program<NftStakingReal>,
  payer: Keypair,
  customEndPoint: string = clusterApiUrl("devnet")
) => {
  const connection = new anchor.web3.Connection(customEndPoint);
  const balance = await connection.getBalance(payer.publicKey);

  // const randomPubkey = anchor.web3.Keypair.generate().publicKey;
  // // send sol to dummy address
  // const transferTx = new Transaction().add(
  //   SystemProgram.transfer({
  //     fromPubkey: payer.publicKey,
  //     toPubkey: randomPubkey,
  //     lamports: 2 * LAMPORTS_PER_SOL,
  //   })
  // );

  // await sendAndConfirmTransaction(connection, transferTx, [payer], {
  //   commitment: "finalized",
  // });

  await safeTransactionHandler(
    connection,
    connection.requestAirdrop(payer.publicKey, 2 * LAMPORTS_PER_SOL)
  );
  const umi = createUmi(customEndPoint);

  const myKeypair = umi.eddsa.createKeypairFromSecretKey(payer.secretKey);

  const myKeypairSigner = createSignerFromKeypair(umi, myKeypair);
  umi
    .use(bundlrUploader())
    .use(signerIdentity(myKeypairSigner))
    .use(mplTokenMetadata());

  let uri: string;
  if (!NFT_URI) {
    // Upload the JSON metadata.

    uri = await umi.uploader.uploadJson({
      name: "Test nft",
      description: "My description",
      sellerFeeBasisPoints: 0,
    });
  } else {
    uri = NFT_URI;
  }

  console.log("URI: ", uri);

  const nftMint = generateSigner(umi);
  await createNft(umi, {
    mint: nftMint,
    name: "Test nft",
    uri,
    symbol: "MJNFT",
    sellerFeeBasisPoints: percentAmount(0, 2),
  }).sendAndConfirm(umi, {
    confirm: {
      commitment: "finalized",
    },
  });

  const asset = await fetchDigitalAsset(umi, nftMint.publicKey);

  // We convert the public keys to web3.js format because anchor uses web3.js
  const nftTokenAccount = await getAssociatedTokenAddress(
    toWeb3JsPublicKey(asset.mint.publicKey),
    payer.publicKey
  );
  const nftMintAddress = toWeb3JsPublicKey(asset.mint.publicKey);
  const nftMasterEditionAddress = toWeb3JsPublicKey(asset.edition.publicKey);

  const [delegatedAuthPda] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("authority")],
    program.programId
  );

  const [stakeStatePda] = anchor.web3.PublicKey.findProgramAddressSync(
    [payer.publicKey.toBuffer(), nftTokenAccount.toBuffer()],
    program.programId
  );

  const [mintAuth] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("mint")],
    program.programId
  );

  console.log("Creating spl token mint...");

  //  SPL reward token mint (NOT NFT)
  const mint = await createMint(
    program.provider.connection,
    payer,
    mintAuth,
    null,
    2,
    undefined,
    {
      commitment: "finalized",
    }
  );

  const tokenAddress = await getAssociatedTokenAddress(mint, payer.publicKey);

  const nft: NftMint = {
    tokenAccount: nftTokenAccount,
    mintAddress: nftMintAddress,
    masterEditionAddress: nftMasterEditionAddress,
  };

  console.log("NFT: ----------------");
  console.log("mint Address: ", nft.mintAddress.toBase58());
  console.log("associated token Address: ", nft.tokenAccount.toBase58());
  console.log("master edition Address: ", nft.masterEditionAddress.toBase58());

  // PDAs
  console.log("Our PDAs");
  console.log("delegated auth Address: ", delegatedAuthPda.toBase58());
  console.log("stake state Address: ", stakeStatePda.toBase58());

  console.log("SPL: -----------------");
  console.log("SPL token address: ", mint.toBase58());
  console.log("SPL token associated address: ", tokenAddress.toBase58());
  return {
    nft,
    delegatedAuthPda: delegatedAuthPda,
    stakeStatePda: stakeStatePda,
    mint: mint,
    mintAuth: mintAuth,
    tokenAddress: tokenAddress,
  };
};
