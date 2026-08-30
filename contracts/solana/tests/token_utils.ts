import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey } from "@solana/web3.js";
import { IdlebotAnchorWorkspace } from "../target/types/idlebot_anchor_workspace";

describe("token_utils", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);

  const program = anchor.workspace
    .idlebotAnchorWorkspace as Program<IdlebotAnchorWorkspace>;

  it("exposes the marketplace + subscription instructions", async () => {
    const idl = await program.fetchIdl();
    expect(idl).not.toBeNull();

    const names = (idl?.instructions ?? []).map((ix) => ix.name);
    expect(names).toEqual(
      expect.arrayContaining([
        "initMarketplace",
        "publishListing",
        "purchaseListing",
        "withdrawListing",
        "getListing",
        "cleanupExpired",
        "initSubscription",
        "purchaseSubscription",
        "refundSubscription",
        "cancelSubscription",
        "withdrawSubscription",
      ])
    );
  });

  it("declares the USDT-style accounts used by the programs", async () => {
    const idl = await program.fetchIdl();
    const accounts = idl?.accounts ?? [];

    const marketplace = accounts.find((a) => a.name === "marketplaceAccount");
    expect(marketplace).toBeDefined();
    expect(marketplace?.type.kind).toBe("struct");

    const subscription = accounts.find(
      (a) => a.name === "subscriptionAccount"
    );
    expect(subscription).toBeDefined();
    expect(subscription?.type.kind).toBe("struct");
  });

  it("can derive the marketplace PDA deterministically", async () => {
    const [pda] = PublicKey.findProgramAddressSync(
      [Buffer.from("marketplace")],
      program.programId
    );
    expect(pda).not.toEqual(PublicKey.default);
  });
});