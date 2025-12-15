<script lang="ts">
    import { goto } from "$app/navigation";
    import { onMount, onDestroy } from "svelte";

    let query = $state("");
    let inputEl: HTMLInputElement;

    function handleSearch(e: Event) {
        e.preventDefault();
        if (query.trim()) {
            goto(`/search?q=${encodeURIComponent(query.trim())}`);
            query = "";
        }
    }

    function handleKeydown(e: KeyboardEvent) {
        // Cmd/Ctrl+K to focus search
        if ((e.metaKey || e.ctrlKey) && e.key === "k") {
            e.preventDefault();
            inputEl?.focus();
        }
    }

    onMount(() => {
        document.addEventListener("keydown", handleKeydown);
    });

    onDestroy(() => {
        document.removeEventListener("keydown", handleKeydown);
    });
</script>

<form onsubmit={handleSearch} class="relative hidden md:flex">
    <div class="relative">
        <svg
            xmlns="http://www.w3.org/2000/svg"
            class="absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground"
            fill="none"
            viewBox="0 0 24 24"
            stroke="currentColor"
        >
            <path
                stroke-linecap="round"
                stroke-linejoin="round"
                stroke-width="2"
                d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
            />
        </svg>
        <input
            bind:this={inputEl}
            bind:value={query}
            type="text"
            placeholder="Search blocks, transactions, addresses..."
            class="h-9 w-64 rounded-md border border-input bg-background pl-9 pr-12 text-sm
                   placeholder:text-muted-foreground focus:outline-none focus:ring-2
                   focus:ring-ring focus:ring-offset-2 focus:ring-offset-background"
        />
        <kbd
            class="pointer-events-none absolute right-2 top-1/2 -translate-y-1/2
                   select-none rounded border border-border bg-muted px-1.5
                   text-[10px] font-medium text-muted-foreground"
        >
            ⌘K
        </kbd>
    </div>
</form>
