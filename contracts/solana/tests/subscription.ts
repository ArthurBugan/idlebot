import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import {
  createMint,
  getOrCreateAssociatedTokenAccount,
  mintTo,
  TOKEN_PROGRAM_ID,
} from "@solana/spl-token";
import { Keypair, PublicKey } from "@solana/web3.js";
import { IdlebotAnchorWorkspace } from "../target/types/idlebot_anchor_workspace";

const USDT_DECIMALS = 6;
const PREMIUM_PRICE = 1_000_000; // 1 USDT
const MONTH_SECONDS = 30 * 24 * 3600;

describe("subscription", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .idlebotAnchorWorkspace as Program<IdlebotAnchorWorkspace>;

  let usdtMint: PublicKey;
  let subscriptionPda: PublicKey;
  let subscriptionAta: PublicKey;

  const user = Keypair.generate();

  before(async () => {
    usdtMint = await createMint(
      provider.connection,
      provider.wallet.payer,
      provider.wallet.publicKey,
      null,
      USDT_DECIMALS
    );
    [subscriptionPda] = PublicKey.findProgramAddressSync(
      [Buffer.from("subscription"), user.publicKey.toBuffer()],
      program.programId
    );
    subscriptionAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      provider.wallet.payer,
      usdtMint,
      subscriptionPda,
      true
    ).then((a) => a.address);
    await provider.connection.confirmTransaction(
      await provider.connection.requestAirdrop(user.publicKey, 5_000_000_000),
      "confirmed"
    );
  });

  it("initializes a free-tier subscription", async () => {
    await program.methods
      .initSubscription()
      .accounts({
        subscription: subscriptionPda,
        authority: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const account = await program.account.subscriptionAccount.fetch(
      subscriptionPda
    );
    expect(account.programId.equals(program.programId)).toBe(true);
    expect(account.owner.equals(user.publicKey)).toBe(true);
    expect(account.limit).toBe(50); // FREE_LIMIT
    expect(account.premiumUntil.eq(new anchor.BN(0))).toBe(true);
  });

  it("purchases premium for 1 USDT / 30 days", async () => {
    const payerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      user,
      usdtMint,
      user.publicKey
    ).then((a) => a.address);
    await mintTo(
      provider.connection,
      user,
      usdtMint,
      payerAta,
      user.publicKey,
      10_000_000
    );

    await program.methods
      .purchaseSubscription(usdtMint, user.publicKey)
      .accounts({
        subscription: subscriptionPda,
        payer: user.publicKey,
        payerAta,
        subscriptionAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const account = await program.account.subscriptionAccount.fetch(
      subscriptionPda
    );
    expect(account.limit).toBe(500); // PREMIUM_LIMIT
    const now = Math.floor(Date.now() / 1000);
    expect(
      account.premiumUntil.gt(new anchor.BN(now)) &&
        account.premiumUntil.lte(new anchor.BN(now + MONTH_SECONDS))
    ).toBe(true);
  });

  it("refunds an active subscription", async () => {
    await program.methods
      .refundSubscription(usdtMint, user.publicKey)
      .accounts({
        subscription: subscriptionPda,
        payer: user.publicKey,
        payerAta: await getOrCreateAssociatedTokenAccount(
          provider.connection,
          user,
          usdtMint,
          user.publicKey
        ).then((a) => a.address),
        subscriptionAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const account = await program.account.subscriptionAccount.fetch(
      subscriptionPda
    );
    expect(account.limit).toBe(50);
    expect(account.premiumUntil.eq(new anchor.BN(0))).toBe(true);
  });

  it("cancels an expired subscription", async () => {
    // Premium never active: premium_until == 0 <= now, so cancel succeeds.
    await program.methods
      .cancelSubscription(usdtMint, user.publicKey)
      .accounts({
        subscription: subscriptionPda,
        payer: user.publicKey,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const account = await program.account.subscriptionAccount.fetch(
      subscriptionPda
    );
    expect(account.limit).toBe(50);
  });

  it("withdraws an active subscription (flat refund)", async () => {
    const payerAta = await getOrCreateAssociatedTokenAccount(
      provider.connection,
      user,
      usdtMint,
      user.publicKey
    ).then((a) => a.address);
    await mintTo(
      provider.connection,
      user,
      usdtMint,
      payerAta,
      user.publicKey,
      10_000_000
    );

    await program.methods
      .purchaseSubscription(usdtMint, user.publicKey)
      .accounts({
        subscription: subscriptionPda,
        payer: user.publicKey,
        payerAta,
        subscriptionAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    await program.methods
      .withdrawSubscription(usdtMint, user.publicKey)
      .accounts({
        subscription: subscriptionPda,
        payer: user.publicKey,
        payerAta,
        subscriptionAta,
        mint: usdtMint,
        tokenProgram: TOKEN_PROGRAM_ID,
        systemProgram: anchor.web3.SystemProgram.programId,
      })
      .signers([user])
      .rpc();

    const account = await program.account.subscriptionAccount.fetch(
      subscriptionPda
    );
    expect(account.limit).toBe(50);
    expect(account.premiumUntil.eq(new anchor.BN(0))).toBe(true);
  });
});