import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { NftStakingReal } from "../target/types/nft_staking_real";
import { NftMint, setupNft } from "../utils/setupNft";
import {
  DigitalAsset,
  MPL_TOKEN_METADATA_PROGRAM_ID,
} from "@metaplex-foundation/mpl-token-metadata";
import { TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { expect } from "chai";

describe("staking initialisation", () => {
  // Configure the client to use the env cluster.
  const provider = anchor.AnchorProvider.env();

  anchor.setProvider(provider);

  const wallet = anchor.workspace.NftStakingReal.provider.wallet;

  let delegatedAuthPda: anchor.web3.PublicKey;
  let stakeStatePda: anchor.web3.PublicKey;
  let nft: NftMint;
  let mintAuth: anchor.web3.PublicKey;
  let mint: anchor.web3.PublicKey;
  let tokenAddress: anchor.web3.PublicKey;

  let program = anchor.workspace.NftStakingReal as Program<NftStakingReal>;

  before(async () => {
    ({ nft, delegatedAuthPda, stakeStatePda, mint, mintAuth, tokenAddress } =
      await setupNft(program, wallet.payer, provider.connection.rpcEndpoint));
  });

  it("Stakes", async () => {
    console.log("Initialising stakes");
    await program.methods
      .stake()
      .accounts({
        nftTokenAccount: nft.tokenAccount,
        nftMint: nft.mintAddress,
        nftEdition: nft.masterEditionAddress,
        metadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
      })
      .rpc();

    const account = await program.account.userStakeInfo.fetch(stakeStatePda);
    console.log("stake status after staking: ", account);
    expect(account.stakeState).to.have.property("staked");
  });

  it("Redeems", async () => {
    console.log("initializing redeem");

    await program.methods
      .redeem()
      .accounts({
        nftTokenAccount: nft.tokenAccount,
        stakeMint: mint,
        userStakeAta: tokenAddress,
      })
      .rpc();

    const stakeAccount = await program.account.userStakeInfo.fetch(
      stakeStatePda
    );
    console.log("stake status after redeem: ", stakeAccount);

    const balance = await program.provider.connection.getTokenAccountBalance(
      tokenAddress
    );
    console.log("Current balance: ", balance);
  });

  it("Unstakes", async () => {
    await program.methods
      .unstake()
      .accounts({
        nftTokenAccount: nft.tokenAccount,
        nftMint: nft.mintAddress,
        nftEdition: nft.masterEditionAddress,
        metadataProgram: MPL_TOKEN_METADATA_PROGRAM_ID,
        stakeMint: mint,
        userStakeAta: tokenAddress,
      })
      .rpc();

    const stakeAccount = await program.account.userStakeInfo.fetch(
      stakeStatePda
    );
    console.log("stake status after unstake: ", stakeAccount);
  });
});
