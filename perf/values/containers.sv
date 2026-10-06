// Value-backend workload: container traffic with narrow and wide packed
// elements: queues, dynamic arrays, associative arrays, sorting and mailbox
// transfers between two processes.

`ifndef LLG_VB_ROUNDS
`define LLG_VB_ROUNDS 40000
`endif

typedef logic [127:0] vb_word_t;

module containers #(
    parameter int ROUNDS = `LLG_VB_ROUNDS
);
    logic [31:0] narrow_q [$];
    vb_word_t wide_q [$];
    logic [63:0] dyn [];
    logic [95:0] assoc [int];
    mailbox #(vb_word_t) channel = new(8);
    logic [127:0] received = '0;
    int got = 0;

    initial begin
        vb_word_t item;
        for (int i = 0; i < ROUNDS; i++) begin
            channel.get(item);
            received = received ^ {item[63:0], item[127:64]};
            got++;
        end
    end

    initial begin
        logic [31:0] narrow_sum;
        vb_word_t wide_sum;
        logic [63:0] dyn_sum;
        logic [95:0] assoc_sum;
        logic [31:0] seed;
        int key;
        seed = 32'h1234_5678;
        narrow_sum = '0;
        wide_sum = '0;
        dyn_sum = '0;
        assoc_sum = '0;
        dyn = new[16];
        for (int i = 0; i < 16; i++) dyn[i] = 64'(i) * 64'h0101_0101;
        for (int i = 0; i < ROUNDS; i++) begin
            seed = seed ^ (seed << 13);
            seed = seed ^ (seed >> 17);
            seed = seed ^ (seed << 5);
            narrow_q.push_back(seed);
            wide_q.push_front({seed, ~seed, seed ^ 32'(i), 32'(i)});
            if (narrow_q.size() > 32) narrow_sum = narrow_sum + narrow_q.pop_front();
            if (wide_q.size() > 24) wide_sum = wide_sum ^ wide_q.pop_back();
            dyn[i % dyn.size()] = dyn[(i + 3) % dyn.size()] + 64'(seed);
            if (i % 512 == 511) dyn = new[dyn.size() + (i % 3)](dyn);
            key = int'(seed[9:0]);
            if (assoc.exists(key)) begin
                assoc_sum = assoc_sum + assoc[key];
                assoc.delete(key);
            end else begin
                assoc[key] = {seed, seed ^ 32'hdead_beef, 32'(i)};
            end
            if (i % 256 == 255) begin
                narrow_q.sort();
                narrow_sum = narrow_sum ^ narrow_q[0] ^ narrow_q[narrow_q.size() - 1];
            end
            channel.put({seed, 32'(i), ~seed, seed + 32'(i)});
        end
        wait (got == ROUNDS);
        foreach (dyn[j]) dyn_sum = dyn_sum ^ dyn[j];
        $display("containers rounds=%0d q=%0d wq=%0d dyn=%0d assoc=%0d sums=%h %h %h %h rx=%h",
                 ROUNDS, narrow_q.size(), wide_q.size(), dyn.size(), assoc.num(), narrow_sum,
                 wide_sum, dyn_sum, assoc_sum, received);
        $finish(0);
    end
endmodule
