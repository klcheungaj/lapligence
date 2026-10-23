
// ── Bidirectional pass switches (IEEE 1800-2009 4.9.5, 28.8, 28.13-28.14) ──
//
// A node is one bit of one net group; tran, tranif0/1, rtran and rtranif0/1
// join two nodes. Every member net resolves through its network: its member
// bits take the network's value and %v strength (`switch_overlay_value`,
// `switch_overlay_strength`), so every existing reader, force overlay, net
// delay and alias refresh sees the network's result.
//
// A solve covers one dynamic component: the nodes reachable from a dirty node
// through switches that are not off (an off switch passes nothing either way,
// so such components are independent). Each node starts from its own
// resolution input, summarised as the strongest known and possible 0 and 1
// levels (K0, K1, P0, P1; the representation of the %v view in nets.c). The
// levels are processed from supply down to small through one FIFO bucket per
// level. A level L of value v leaves its node only if no stronger known
// opposite value is there (L >= K_{!v}): a node passes on only what survives
// its own resolution. It reaches a neighbour at the reduced level (supply to
// strong across tran*, Table 28-8 across rtran*); across a switch whose state
// is unknown it arrives as possible only, which yields the union of the
// conducting and nonconducting cases that 4.9.5 describes, strengths
// included. A forced bit is a fixed source with its forced value at strong
// strength and takes no arrivals.
//
// Termination: levels only rise, items are taken in non-increasing level
// order and a reduction never raises a level, so the first raise of an entry
// is its final level. Each of a node's four entries is raised at most once
// after its own value, so a solve pushes at most eight items per node and
// costs O(nodes + switches) of its component; no iteration count is involved.

#define LLG_SWITCH_ENTRIES 4u
#define LLG_SWITCH_ITEMS_PER_NODE 8u
#define LLG_SWITCH_NO_ITEM UINT32_MAX

enum { LLG_SWITCH_OFF = 0, LLG_SWITCH_ON = 1, LLG_SWITCH_UNKNOWN = 2 };

typedef struct {
    uint32_t bit;
    uint32_t node;
} llg_switch_member_t;

// The member bits of one net, sorted by bit, and whether the net waits in
// the publication list.
struct llg_switch_net {
    uint32_t count;
    uint8_t queued;
    llg_switch_member_t members[];
};

typedef struct {
    uint32_t node;
    uint32_t next;
    uint8_t entry;
} llg_switch_item_t;

static struct {
    const llg_switch_node_t* nodes;
    uint32_t n_nodes;
    const llg_switch_t* switches;
    uint32_t n_switches;
    // CSR adjacency: the switches of node n are adj[adj_start[n]..adj_start[n+1]).
    uint32_t* adj_start;
    uint32_t* adj;
    uint8_t* state;
    // Per node: K0, K1, P0, P1 (-1 none, 1..7 small..supply).
    int8_t* level;
    // Per node: last result, 0/1/2 (x)/3 (z), and its strength view code.
    uint8_t* value;
    uint8_t* code;
    uint8_t* fixed;
    uint8_t* dirty;
    uint32_t* mark;
    uint32_t epoch;
    uint32_t* dirty_list;
    uint32_t n_dirty;
    uint32_t* component;
    llg_switch_item_t* items;
    uint32_t n_items;
    uint32_t head[LLG_STRENGTH_SUPPLY + 1];
    uint32_t tail[LLG_STRENGTH_SUPPLY + 1];
    llg_net_t** nets;
    uint32_t n_nets;
    llg_net_t** publish;
    uint32_t n_publish;
    int solving;
} g_sw;

static void switches_release(void) {
    for (uint32_t i = 0; i < g_sw.n_nets; i++) {
        free(g_sw.nets[i]->switch_net);
        g_sw.nets[i]->switch_net = NULL;
    }
    free(g_sw.nets);
    free(g_sw.adj_start);
    free(g_sw.adj);
    free(g_sw.state);
    free(g_sw.level);
    free(g_sw.value);
    free(g_sw.code);
    free(g_sw.fixed);
    free(g_sw.dirty);
    free(g_sw.mark);
    free(g_sw.dirty_list);
    free(g_sw.component);
    free(g_sw.items);
    free(g_sw.publish);
    memset(&g_sw, 0, sizeof(g_sw));
}

// Table 28-8 for resistive devices; otherwise supply becomes strong (28.13).
static int switch_reduce(int level, uint8_t flags) {
    static const int8_t resistive[LLG_STRENGTH_SUPPLY + 1] = {0, 1, 1, 2, 2, 3, 5, 5};
    if (flags & LLG_SWITCH_RESISTIVE) return resistive[level];
    return level == LLG_STRENGTH_SUPPLY ? LLG_STRENGTH_STRONG : level;
}

static void switch_raise(int8_t* entry, int level) {
    if (level > *entry) *entry = (int8_t)level;
}

// The node's own resolution input: its net's drivers on its bit, the
// implicit pull or supply source, and StX for a bit whose drivers have not
// evaluated yet (decision T0-D3). A forced bit is a fixed strong source.
static void switch_own_levels(uint32_t node) {
    const llg_switch_node_t* entry = &g_sw.nodes[node];
    llg_net_t* net = entry->net;
    uint32_t bit = entry->bit;
    int8_t* level = &g_sw.level[(size_t)node * LLG_SWITCH_ENTRIES];
    for (unsigned i = 0; i < LLG_SWITCH_ENTRIES; i++) level[i] = -1;
    g_sw.fixed[node] = 0;
    if (llg_is_forced(&net->resolved) &&
        (force_mask_word(&net->resolved, bit / 64u) >> (bit % 64u) & 1u)) {
        g_sw.fixed[node] = 1;
        switch (llg_sv4_state(net->resolved, bit)) {
        case 0: level[0] = level[2] = LLG_STRENGTH_STRONG; break;
        case 1: level[1] = level[3] = LLG_STRENGTH_STRONG; break;
        case 2: level[2] = level[3] = LLG_STRENGTH_STRONG; break;
        default: break;
        }
        return;
    }
    switch (net->resolution) {
    case LLG_RESOLVE_TRI0: level[0] = level[2] = LLG_STRENGTH_PULL; break;
    case LLG_RESOLVE_TRI1: level[1] = level[3] = LLG_STRENGTH_PULL; break;
    case LLG_RESOLVE_SUPPLY0: level[0] = level[2] = LLG_STRENGTH_SUPPLY; break;
    case LLG_RESOLVE_SUPPLY1: level[1] = level[3] = LLG_STRENGTH_SUPPLY; break;
    default: break;
    }
    int count = net->n_drivers;
    const int* drivers = NULL;
    if (llg_net_has_index(net)) {
        count = 0;
        llg_net_index_collect(net, net->index_root, bit, bit, &count);
        drivers = net->overlap_scratch;
    }
    for (int i = 0; i < count; i++) {
        int d = drivers ? drivers[i] : i;
        const sv4_t* v = net->drivers[d];
        if (!v || bit >= llg_sv4_width(*v)) continue;
        int s0 = net->strength0 ? net->strength0[d] : LLG_STRENGTH_STRONG;
        int s1 = net->strength1 ? net->strength1[d] : LLG_STRENGTH_STRONG;
        switch (llg_sv4_state(*v, bit)) {
        case 0:
            if (s0 != LLG_STRENGTH_HIGHZ) {
                switch_raise(&level[0], s0);
                switch_raise(&level[2], s0);
            }
            break;
        case 1:
            if (s1 != LLG_STRENGTH_HIGHZ) {
                switch_raise(&level[1], s1);
                switch_raise(&level[3], s1);
            }
            break;
        case 2:
            if (s0 != LLG_STRENGTH_HIGHZ) switch_raise(&level[2], s0);
            if (s1 != LLG_STRENGTH_HIGHZ) switch_raise(&level[3], s1);
            break;
        default: break;
        }
    }
    if (net->start_pending && (net->start_pending[bit / 64u] >> (bit % 64u) & 1u)) {
        switch_raise(&level[2], LLG_STRENGTH_STRONG);
        switch_raise(&level[3], LLG_STRENGTH_STRONG);
    }
}

static void switch_push(uint32_t node, unsigned entry, int level) {
    if (g_sw.n_items >= (size_t)g_sw.n_nodes * LLG_SWITCH_ITEMS_PER_NODE) {
        fputs("llg runtime fatal: switch network item bound exceeded\n", stderr);
        abort();
    }
    uint32_t index = g_sw.n_items++;
    g_sw.items[index] = (llg_switch_item_t){node, LLG_SWITCH_NO_ITEM, (uint8_t)entry};
    if (g_sw.head[level] == LLG_SWITCH_NO_ITEM)
        g_sw.head[level] = index;
    else
        g_sw.items[g_sw.tail[level]].next = index;
    g_sw.tail[level] = index;
}

// Offer level `level` of value side `side` (known or possible) to `node`.
static void switch_offer(uint32_t node, unsigned side, int known, int level) {
    if (g_sw.fixed[node] || level < 1) return;
    int8_t* entries = &g_sw.level[(size_t)node * LLG_SWITCH_ENTRIES];
    if (known) {
        if (level <= entries[side]) return;
        entries[side] = (int8_t)level;
        switch_raise(&entries[2u + side], level);
        switch_push(node, side, level);
    } else {
        if (level <= entries[2u + side]) return;
        entries[2u + side] = (int8_t)level;
        switch_push(node, 2u + side, level);
    }
}

static uint32_t switch_other(uint32_t s, uint32_t node) {
    return g_sw.switches[s].a == node ? g_sw.switches[s].b : g_sw.switches[s].a;
}

static void switch_queue_publish(llg_net_t* net) {
    struct llg_switch_net* members = net->switch_net;
    if (!members || members->queued) return;
    members->queued = 1;
    g_sw.publish[g_sw.n_publish++] = net;
}

// Solve the dynamic component of `start` and record each node's result.
static void switch_solve(uint32_t start) {
    if (++g_sw.epoch == 0) {
        memset(g_sw.mark, 0, (size_t)g_sw.n_nodes * sizeof(*g_sw.mark));
        g_sw.epoch = 1;
    }
    uint32_t count = 0;
    g_sw.component[count++] = start;
    g_sw.mark[start] = g_sw.epoch;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t node = g_sw.component[i];
        for (uint32_t k = g_sw.adj_start[node]; k < g_sw.adj_start[node + 1u]; k++) {
            uint32_t s = g_sw.adj[k];
            if (g_sw.state[s] == LLG_SWITCH_OFF) continue;
            uint32_t other = switch_other(s, node);
            if (g_sw.mark[other] == g_sw.epoch) continue;
            g_sw.mark[other] = g_sw.epoch;
            g_sw.component[count++] = other;
        }
    }
    g_sw.n_items = 0;
    for (int level = 0; level <= LLG_STRENGTH_SUPPLY; level++)
        g_sw.head[level] = g_sw.tail[level] = LLG_SWITCH_NO_ITEM;
    for (uint32_t i = 0; i < count; i++) {
        uint32_t node = g_sw.component[i];
        g_sw.dirty[node] = 0;
        switch_own_levels(node);
        const int8_t* entries = &g_sw.level[(size_t)node * LLG_SWITCH_ENTRIES];
        for (unsigned entry = 0; entry < LLG_SWITCH_ENTRIES; entry++)
            if (entries[entry] >= 1) switch_push(node, entry, entries[entry]);
    }
    for (int level = LLG_STRENGTH_SUPPLY; level >= 1; level--) {
        while (g_sw.head[level] != LLG_SWITCH_NO_ITEM) {
            llg_switch_item_t item = g_sw.items[g_sw.head[level]];
            g_sw.head[level] = item.next;
            const int8_t* entries = &g_sw.level[(size_t)item.node * LLG_SWITCH_ENTRIES];
            unsigned side = item.entry & 1u;
            int known = item.entry < 2u;
            // Stale: the entry rose after this item was queued.
            if (entries[item.entry] != level) continue;
            // Blocked: a stronger known opposite value wins at this node.
            if (entries[1u - side] > level) continue;
            for (uint32_t k = g_sw.adj_start[item.node]; k < g_sw.adj_start[item.node + 1u]; k++) {
                uint32_t s = g_sw.adj[k];
                uint8_t state = g_sw.state[s];
                if (state == LLG_SWITCH_OFF) continue;
                switch_offer(switch_other(s, item.node), side,
                             known && state == LLG_SWITCH_ON,
                             switch_reduce(level, g_sw.switches[s].flags));
            }
        }
    }
    for (uint32_t i = 0; i < count; i++) {
        uint32_t node = g_sw.component[i];
        if (g_sw.fixed[node]) continue;
        const int8_t* e = &g_sw.level[(size_t)node * LLG_SWITCH_ENTRIES];
        llg_net_t* net = g_sw.nodes[node].net;
        uint8_t code = llg_net_strength_hull(net->resolution, e[0], e[1], e[2], e[3]);
        int lo = (int)(code & 15u) - 7;
        int hi = (int)(code >> 4) - 7;
        uint8_t value = lo == 0 && hi == 0 ? 3u : hi < 0 ? 0u : lo > 0 ? 1u : 2u;
        if (value == g_sw.value[node] && code == g_sw.code[node]) continue;
        g_sw.value[node] = value;
        g_sw.code[node] = code;
        switch_queue_publish(net);
    }
}

// Solve every dirty component and republish the queued nets until nothing is
// left; a callback reached from a publication only queues more work.
static int switches_flush(void) {
    if (g_sw.solving || (!g_sw.n_dirty && !g_sw.n_publish)) return 0;
    g_sw.solving = 1;
    while (g_sw.n_dirty || g_sw.n_publish) {
        while (g_sw.n_dirty) {
            uint32_t node = g_sw.dirty_list[--g_sw.n_dirty];
            if (g_sw.dirty[node]) switch_solve(node);
        }
        while (g_sw.n_publish && !g_sw.n_dirty) {
            llg_net_t* net = g_sw.publish[--g_sw.n_publish];
            net->switch_net->queued = 0;
            llg_net_resolve(net);
        }
    }
    g_sw.solving = 0;
    return 1;
}

static void switch_mark_dirty(uint32_t node) {
    if (g_sw.dirty[node]) return;
    g_sw.dirty[node] = 1;
    g_sw.dirty_list[g_sw.n_dirty++] = node;
}

// Inside a process turn the network waits for the end of the region queue
// drain, like a net with several drivers (decision S2-D11); otherwise it is
// solved at once.
static void switches_schedule(void) {
    if (!net_defer_active) switches_flush();
}

// A driver slot of `net` changed, or a force or release changed its forced
// bits: its nodes are re-solved and the net is republished through them.
static void llg_switch_net_changed(llg_net_t* net) {
    struct llg_switch_net* members = net->switch_net;
    if (!members) return;
    for (uint32_t i = 0; i < members->count; i++)
        switch_mark_dirty(members->members[i].node);
    switch_queue_publish(net);
    switches_schedule();
}

void llg_switch_control(uint32_t index, sv4_t state) {
    if (index >= g_sw.n_switches) {
        fputs("llg runtime fatal: invalid switch index\n", stderr);
        abort();
    }
    if (!region_can_mutate("switch control")) return;
    uint8_t next = LLG_SWITCH_UNKNOWN;
    if (llg_sv4_width(state)) {
        unsigned bit = llg_sv4_state(state, 0);
        if (bit == 0) next = LLG_SWITCH_OFF;
        if (bit == 1) next = LLG_SWITCH_ON;
    }
    if (g_sw.state[index] == next) return;
    g_sw.state[index] = next;
    switch_mark_dirty(g_sw.switches[index].a);
    switch_mark_dirty(g_sw.switches[index].b);
    switches_schedule();
}

static int switch_member_order(const void* left, const void* right) {
    const llg_switch_member_t* a = (const llg_switch_member_t*)left;
    const llg_switch_member_t* b = (const llg_switch_member_t*)right;
    return a->bit < b->bit ? -1 : a->bit > b->bit;
}

void llg_switches_start(const llg_switch_node_t* nodes, uint32_t n_nodes,
                        const llg_switch_t* switches, uint32_t n_switches) {
    switches_release();
    if (!nodes || !n_nodes) return;
    if (n_nodes > UINT32_MAX / LLG_SWITCH_ITEMS_PER_NODE ||
        n_switches > UINT32_MAX / 2u) {
        fputs("llg runtime fatal: switch network is too large\n", stderr);
        abort();
    }
    g_sw.nodes = nodes;
    g_sw.n_nodes = n_nodes;
    g_sw.switches = switches;
    g_sw.n_switches = n_switches;
    g_sw.adj_start = (uint32_t*)llg_checked_calloc((size_t)n_nodes + 1u, sizeof(uint32_t),
                                                 "switch adjacency");
    g_sw.adj = (uint32_t*)llg_checked_calloc((size_t)n_switches * 2u + 1u, sizeof(uint32_t),
                                           "switch adjacency");
    g_sw.state = (uint8_t*)llg_checked_calloc((size_t)n_switches + 1u, 1, "switch states");
    g_sw.level = (int8_t*)llg_checked_calloc((size_t)n_nodes * LLG_SWITCH_ENTRIES, 1,
                                           "switch levels");
    g_sw.value = (uint8_t*)llg_checked_calloc(n_nodes, 1, "switch values");
    g_sw.code = (uint8_t*)llg_checked_calloc(n_nodes, 1, "switch strengths");
    g_sw.fixed = (uint8_t*)llg_checked_calloc(n_nodes, 1, "switch forced nodes");
    g_sw.dirty = (uint8_t*)llg_checked_calloc(n_nodes, 1, "switch dirty nodes");
    g_sw.mark = (uint32_t*)llg_checked_calloc(n_nodes, sizeof(uint32_t), "switch marks");
    g_sw.dirty_list = (uint32_t*)llg_checked_calloc(n_nodes, sizeof(uint32_t),
                                                  "switch dirty list");
    g_sw.component = (uint32_t*)llg_checked_calloc(n_nodes, sizeof(uint32_t),
                                                 "switch component");
    g_sw.items = (llg_switch_item_t*)llg_checked_calloc(
        (size_t)n_nodes * LLG_SWITCH_ITEMS_PER_NODE, sizeof(llg_switch_item_t),
        "switch solve queue");
    for (uint32_t s = 0; s < n_switches; s++) {
        if (switches[s].a >= n_nodes || switches[s].b >= n_nodes) {
            fputs("llg runtime fatal: invalid switch terminal\n", stderr);
            abort();
        }
        g_sw.adj_start[switches[s].a + 1u]++;
        g_sw.adj_start[switches[s].b + 1u]++;
        g_sw.state[s] = switches[s].flags & LLG_SWITCH_CONTROLLED ? LLG_SWITCH_UNKNOWN
                                                                 : LLG_SWITCH_ON;
    }
    for (uint32_t n = 0; n < n_nodes; n++) g_sw.adj_start[n + 1u] += g_sw.adj_start[n];
    uint32_t* fill = (uint32_t*)llg_checked_calloc(n_nodes, sizeof(uint32_t), "switch adjacency");
    for (uint32_t s = 0; s < n_switches; s++) {
        g_sw.adj[g_sw.adj_start[switches[s].a] + fill[switches[s].a]++] = s;
        g_sw.adj[g_sw.adj_start[switches[s].b] + fill[switches[s].b]++] = s;
    }
    // Member lists: count the nodes of each net, then fill and sort them.
    llg_net_t** nets = (llg_net_t**)llg_checked_calloc(n_nodes, sizeof(*nets), "switch nets");
    uint32_t n_nets = 0;
    for (uint32_t n = 0; n < n_nodes; n++) {
        llg_net_t* net = nodes[n].net;
        if (!net || nodes[n].bit >= net->width) {
            fputs("llg runtime fatal: invalid switch node\n", stderr);
            abort();
        }
        if (net->switch_net) {
            net->switch_net->count++;
            continue;
        }
        net->switch_net = (struct llg_switch_net*)llg_checked_calloc(
            1, sizeof(struct llg_switch_net), "switch net members");
        net->switch_net->count = 1;
        nets[n_nets++] = net;
    }
    for (uint32_t i = 0; i < n_nets; i++) {
        uint32_t members = nets[i]->switch_net->count;
        free(nets[i]->switch_net);
        nets[i]->switch_net = (struct llg_switch_net*)llg_checked_calloc(
            1, sizeof(struct llg_switch_net) + (size_t)members * sizeof(llg_switch_member_t),
            "switch net members");
    }
    for (uint32_t n = 0; n < n_nodes; n++) {
        struct llg_switch_net* members = nodes[n].net->switch_net;
        members->members[members->count++] = (llg_switch_member_t){nodes[n].bit, n};
    }
    for (uint32_t i = 0; i < n_nets; i++)
        qsort(nets[i]->switch_net->members, nets[i]->switch_net->count,
              sizeof(llg_switch_member_t), switch_member_order);
    free(fill);
    g_sw.nets = nets;
    g_sw.n_nets = n_nets;
    g_sw.publish = (llg_net_t**)llg_checked_calloc((size_t)n_nets + 1u, sizeof(*g_sw.publish),
                                                 "switch publication list");
    // Start every node from its net's current value, then solve everything.
    for (uint32_t n = 0; n < n_nodes; n++) {
        g_sw.value[n] = (uint8_t)llg_sv4_state(nodes[n].net->resolved, nodes[n].bit);
        g_sw.code[n] = 0xFFu;
        switch_mark_dirty(n);
    }
    for (uint32_t i = 0; i < n_nets; i++) switch_queue_publish(nets[i]);
    switches_flush();
}

// The member bits of `net` take their network's values.
static void switch_overlay_value(const llg_net_t* net, sv4_t* value, uint32_t low,
                                 uint32_t width) {
    const struct llg_switch_net* members = net->switch_net;
    for (uint32_t i = 0; i < members->count; i++) {
        uint32_t bit = members->members[i].bit;
        if (bit < low || bit - low >= width) continue;
        llg_sv4_range_fill(value, bit - low, 1u, g_sw.value[members->members[i].node]);
    }
}

// The member bits of `net` take their network's strengths, except forced bits.
static void switch_overlay_strength(llg_net_t* net, uint64_t* out) {
    const struct llg_switch_net* members = net->switch_net;
    int forced = llg_is_forced(&net->resolved);
    for (uint32_t i = 0; i < members->count; i++) {
        uint32_t bit = members->members[i].bit;
        if (forced && (force_mask_word(&net->resolved, bit / 64u) >> (bit % 64u) & 1u))
            continue;
        uint8_t code = g_sw.code[members->members[i].node];
        if (code == 0xFFu) continue;
        out[bit / 8u] = (out[bit / 8u] & ~(UINT64_C(0xFF) << ((bit % 8u) * 8u))) |
                        ((uint64_t)code << ((bit % 8u) * 8u));
    }
}
