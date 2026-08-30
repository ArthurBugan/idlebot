import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  createAccount,
  mintTo,
  getOrCreateAssociatedTokenAccount,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { IdlebotAnchorWorkspace } from "../target/types/idlebot_anchor_workspace";

const USDT_DECIMALS = 6;

describe("marketplace", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .idlebotAnchorWorkspace as Program<IdlebotAnchorWorkspace>;

  let usdtMint: PublicKey;
  let marketplacePda: PublicKey;
  let marketplaceBump: number;
  let marketplaceAta: PublicKey;
  let platformFeeAta: PublicKey;

  const publisher = Keypair.generate();
  const buyer = Keypair.generate();
  const feeWallet = Keypair.generate();

  const publishUsdt = async (owner: Keypair, amount: number) => {
    const ata = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      owner,
      usdtMint,
      owner.publicKey
    );
    await mintTo(
      provider.connection,
      owner,
      usdtMint,
      ata.address,
      owner.publicKey,
      amount
    );
    return ata.address;
  };

  before(async () => {
    usdtMint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      USDT_DECIMALS
    );
    [marketplacePda, marketplaceBump] = PublicKey.findProgramAddressSync(
      [Buffer.from("marketplace")],
      program.programId
    );
    marketplaceAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      usdtMint,
      marketplacePda,
      true
    ).then((a) => a.address);
    platformFeeAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      usdtMint,
      feeWallet.publicKey
    ).then((a) => a.address);
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        publisher.publicKey,
        5_000_000_000
      ),
      "confirmed"
    );
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(buyer.publicKey, 5_000_000_000),
      "confirmed"
    );
  });

  it("initializes the marketplace PDA", async () => {
    await program.methods
      .initMarketplace()
      .accounts({
        marketplace: marketplacePda,
        authority: provider.wallet.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .rpc();

    const account = await program.account.marketplaceAccount.fetch(
      marketplacePda
    );
    expect(account.programId.equals(program.programId)).toBe(true);
    expect(account.marketAuthority.equals(provider.wallet.publicKey)).toBe(
      true
    );
    expect(account.listings.length).toBe(0);
  });

  it("publishes a listing (50 USDT fee)", async () => {
    const publisherAta = await publishUsdt(publisher, 100_000_000);
    const listingId = new anchor.BN(1);
    const title = "Tile pack #1";
    const url = "https://github.com/idlebot/tile-pack-1";
    const description = "128 isometric tiles";
    const price = new anchor.BN(25_000_000); // 25 USDT

    await program.methods
      .publishListing(listingId, title, url, description, price)
      .accounts({
        marketplace: marketplacePda,
        publisher: publisher.publicKey,
        publisherAta,
        marketplaceAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([publisher])
      .rpc();

    const account = await program.account.marketplaceAccount.fetch(
      marketplacePda
    );
    expect(account.listings.length).toBe(1);
    expect(account.listings[0].listingId.eq(listingId)).toBe(true);
    expect(account.listings[0].seller.equals(publisher.publicKey)).toBe(true);
    expect(account.listings[0].sold).toBe(false);

    const feeBalance = await provider.connection.getTokenAccountBalance(
      marketplaceAta
    );
    expect(feeBalance.value.amount).toBe("50000000");
  });

  it("rejects a duplicate listing id", async () => {
    const secondPublisher = Keypair.generate();
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(
        secondPublisher.publicKey,
        5_000_000_000
      ),
      "confirmed"
    );
    const ata = await publishUsdt(secondPublisher, 100_000_000);

    await expect(
      program.methods
        .publishListing(
          new anchor.BN(1),
          "dup",
          "https://github.com/idlebot/dup",
          "dup",
          new anchor.BN(25_000_000)
        )
        .accounts({
          marketplace: marketplacePda,
          publisher: secondPublisher.publicKey,
          publisherAta: ata,
          marketplaceAta,
          mint: usdtMint,
          tokenProgram: TOKEN_PROGRAM_ID,
          associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
          systemProgram: anchor.web3.SystemProgram.programId,
        })
        .signers([secondPublisher])
        .rpc()
    ).rejects.toThrow();
  });

  it("purchases a listing (5% platform fee)", async () => {
    const buyerAta = await publishUsdt(buyer, 100_000_000);
    const sellerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      usdtMint,
      publisher.publicKey
    ).then((a) => a.address);

    await program.methods
      .purchaseListing(new anchor.BN(1))
      .accounts({
        marketplace: marketplacePda,
        buyer: buyer.publicKey,
        buyerAta,
        marketplaceAta,
        platformFeeAta,
        sellerAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([buyer])
      .rpc();

    const account = await program.account.marketplaceAccount.fetch(
      marketplacePda
    );
    expect(account.listings[0].sold).toBe(true);
    expect(account.listings[0].buyer.equals(buyer.publicKey)).toBe(true);

    // Seller received price - 5% = 23.75 USDT; platform got 1.25 USDT.
    const sellerBal = await provider.connection.getTokenAccountBalance(
      sellerAta
    );
    expect(sellerBal.value.amount).toBe("23750000");
    const feeBal = await provider.connection.getTokenAccountBalance(
      platformFeeAta
    );
    expect(feeBal.value.amount).toBe("1250000");
  });

  it("withdraws the proceeds (listing re-listed)", async () => {
    const sellerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      buyer,
      usdtMint,
      publisher.publicKey
    ).then((a) => a.address);

    await program.methods
      .withdrawListing(new anchor.BN(1))
      .accounts({
        marketplace: marketplacePda,
        seller: publisher.publicKey,
        marketplaceAta,
        sellerAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        associatedTokenProgram: anchor.utils.token.ASSOCIATED_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([publisher])
      .rpc();

    const account = await program.account.marketplaceAccount.fetch(
      marketplacePda
    );
    expect(account.listings[0].sold).toBe(false);
    expect(
      account.listings[0].buyer.equals(PublicKey.default)
    ).toBe(true);
  });
});