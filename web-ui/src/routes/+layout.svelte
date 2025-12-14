<script lang="ts">
  import "../app.css";
  import { page } from "$app/stores";
  import { Button } from "$lib/components/ui/button";
  import { Separator } from "$lib/components/ui/separator";

  const navItems = [
    { href: "/", label: "Dashboard", icon: "📊" },
    { href: "/blocks", label: "Blocks", icon: "🧱" },
    { href: "/wallets", label: "Wallets", icon: "👛" },
    { href: "/mining", label: "Mining", icon: "⛏️" },
    { href: "/contracts", label: "Contracts", icon: "📜" },
    { href: "/mempool", label: "Mempool", icon: "📬" },
  ];
</script>

<div class="dark min-h-screen bg-background text-foreground">
  <!-- Header -->
  <header
    class="sticky top-0 z-50 w-full border-b border-border/40 bg-background/95 backdrop-blur supports-[backdrop-filter]:bg-background/60"
  >
    <div class="container flex h-14 max-w-screen-2xl items-center">
      <div class="mr-4 flex">
        <a href="/" class="mr-6 flex items-center space-x-2">
          <span class="text-xl">⛓️</span>
          <span class="font-bold inline-block">mini-blockchain</span>
        </a>
        <nav class="flex items-center gap-4 text-sm lg:gap-6">
          {#each navItems as item}
            <a
              href={item.href}
              class="transition-colors hover:text-foreground/80 {$page.url
                .pathname === item.href
                ? 'text-foreground font-medium'
                : 'text-foreground/60'}"
            >
              {item.label}
            </a>
          {/each}
        </nav>
      </div>
      <div class="flex flex-1 items-center justify-end space-x-2">
        <Button variant="outline" size="sm" class="hidden md:flex">
          <span class="text-green-500 mr-2">●</span>
          Connected
        </Button>
      </div>
    </div>
  </header>

  <!-- Main Content -->
  <main class="flex-1">
    <div class="container max-w-screen-2xl py-6">
      <slot />
    </div>
  </main>

  <!-- Footer -->
  <footer class="border-t border-border/40 py-6 md:py-0">
    <div
      class="container flex flex-col items-center justify-between gap-4 md:h-16 md:flex-row"
    >
      <p
        class="text-balance text-center text-sm leading-loose text-muted-foreground md:text-left"
      >
        Built with
        <a
          href="https://www.rust-lang.org/"
          target="_blank"
          class="font-medium underline underline-offset-4">Rust</a
        >
        +
        <a
          href="https://svelte.dev"
          target="_blank"
          class="font-medium underline underline-offset-4">Svelte</a
        >
        +
        <a
          href="https://shadcn-svelte.com"
          target="_blank"
          class="font-medium underline underline-offset-4">shadcn-svelte</a
        >
      </p>
    </div>
  </footer>
</div>
