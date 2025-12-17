<script lang="ts">
    import {
        mineBlock,
        getWallets,
        type MineResponse,
        type WalletResponse,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";
    import { onMount } from "svelte";

    let wallets = $state<WalletResponse[]>([]);
    let minerAddress = $state("");
    let mining = $state(false);
    let result = $state<MineResponse | null>(null);
    let error = $state("");

    onMount(async () => {
        try {
            wallets = await getWallets();
            if (wallets.length > 0) {
                minerAddress = wallets[0].address;
            }
        } catch (e) {
            console.error(e);
        }
    });

    async function handleMine() {
        if (!minerAddress) return;
        mining = true;
        error = "";
        result = null;

        try {
            result = await mineBlock(minerAddress);
        } catch (e) {
            error = "Mining failed. Please try again.";
        } finally {
            mining = false;
        }
    }

    function setMinerAddress(addr: string) {
        minerAddress = addr;
    }
</script>

<svelte:head>
    <title>Mining | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Mining</h1>
        <p class="text-muted-foreground">Mine new blocks and earn rewards</p>
    </div>

    <div class="grid gap-6 lg:grid-cols-2">
        <!-- Mining Controls -->
        <Card.Root>
            <Card.Header>
                <Card.Title>⛏️ Mine a Block</Card.Title>
                <Card.Description
                    >Start mining to add a new block to the chain</Card.Description
                >
            </Card.Header>
            <Card.Content class="space-y-4">
                <div class="space-y-2">
                    <Label for="address">Miner Address</Label>
                    <Input
                        id="address"
                        bind:value={minerAddress}
                        placeholder="Enter your wallet address"
                        class="font-mono text-sm"
                    />
                </div>

                {#if wallets.length > 0}
                    <div class="space-y-2">
                        <p class="text-sm text-muted-foreground">
                            Quick select:
                        </p>
                        <div class="flex flex-wrap gap-2">
                            {#each wallets.slice(0, 3) as wallet}
                                <Button
                                    variant="outline"
                                    size="sm"
                                    onclick={() =>
                                        setMinerAddress(wallet.address)}
                                >
                                    {wallet.address.slice(0, 12)}...
                                </Button>
                            {/each}
                        </div>
                    </div>
                {/if}

                {#if error}
                    <p class="text-sm text-destructive">{error}</p>
                {/if}
            </Card.Content>
            <Card.Footer>
                <Button
                    class="w-full"
                    size="lg"
                    onclick={() => handleMine()}
                    disabled={mining || !minerAddress}
                >
                    {#if mining}
                        <span
                            class="mr-2 h-4 w-4 animate-spin rounded-full border-2 border-background border-t-transparent"
                        ></span>
                        Mining...
                    {:else}
                        ⛏️ Start Mining
                    {/if}
                </Button>
            </Card.Footer>
        </Card.Root>

        <!-- Mining Result -->
        <Card.Root>
            <Card.Header>
                <Card.Title>Mining Result</Card.Title>
                <Card.Description
                    >Results of your mining operation</Card.Description
                >
            </Card.Header>
            <Card.Content>
                {#if result}
                    <div class="space-y-6">
                        <div
                            class="rounded-lg border border-green-500/50 bg-green-500/10 p-4"
                        >
                            <p
                                class="text-green-500 font-medium flex items-center gap-2"
                            >
                                <span>✅</span> Block Mined Successfully!
                            </p>
                        </div>

                        <!-- Coinbase Maturity Warning -->
                        <div
                            class="rounded-lg border border-yellow-500/50 bg-yellow-500/10 p-3"
                        >
                            <p
                                class="text-yellow-500 text-sm flex items-center gap-2"
                            >
                                <span>⏳</span> <strong>Note:</strong> Reward requires
                                100 block confirmations before it can be spent
                            </p>
                        </div>

                        <div class="grid grid-cols-2 gap-4">
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Block
                                </p>
                                <p class="text-2xl font-bold">
                                    #{result.block.index}
                                </p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Reward
                                </p>
                                <p class="text-2xl font-bold text-green-500">
                                    {result.reward}
                                </p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Time
                                </p>
                                <p class="font-mono">{result.time_ms}ms</p>
                            </div>
                            <div class="space-y-1">
                                <p class="text-sm text-muted-foreground">
                                    Attempts
                                </p>
                                <p class="font-mono">
                                    {result.attempts.toLocaleString()}
                                </p>
                            </div>
                        </div>

                        <Separator />

                        <div class="space-y-2">
                            <p class="text-sm text-muted-foreground">
                                Block Hash
                            </p>
                            <code
                                class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                >{result.block.hash}</code
                            >
                        </div>
                    </div>
                {:else}
                    <div
                        class="flex flex-col items-center justify-center h-64 text-muted-foreground"
                    >
                        <span class="text-5xl mb-4">⛏️</span>
                        <p class="font-medium">Ready to mine</p>
                        <p class="text-sm mt-1">
                            Click "Start Mining" to mine a block
                        </p>
                        <Badge variant="secondary" class="mt-4"
                            >Reward: 50 coins</Badge
                        >
                    </div>
                {/if}
            </Card.Content>
        </Card.Root>
    </div>
</div>
