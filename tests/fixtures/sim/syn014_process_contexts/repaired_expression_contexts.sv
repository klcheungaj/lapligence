// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/repaired_expression_contexts.sv
// IEEE 1800-2009 §§9.2.2.2.1, 9.2.2.3, 9.2.2.4, 11.4.5, 11.4.11, 11.4.13,
// 12.6 and 10.9.1; IEEE 1364-2001 §9.7.5: sensitivity and writer contracts
// for expression kinds admitted by the fixed-value and pattern repairs.
module tb;
    typedef struct { logic [7:0] data; bit flag; logic [3:0] row[2]; } record_t;
    typedef logic [7:0] lane_t;
    typedef lane_t row_t [1:0];
    typedef union tagged packed { void invalid; logic [7:0] valid; } maybe_t;
    localparam record_t REFERENCE = '{data:8'h5a, flag:1, row:'{4'h1, 4'h2}};

    record_t left_record, right_record, merged, registered;
    logic choose_left;
    row_t table_row, source_row;
    lane_t key;
    logic hit;
    logic equal_reference;
    maybe_t maybe;
    lane_t threshold, matched;
    lane_t first_lane, second_lane;
    lane_t memory [0:3];
    logic [1:0] address;
    lane_t star_word;
    lane_t latched;
    logic latch_open;
    logic clk;
    integer merge_runs = 0;
    integer downstream_runs = 0;
    integer runs_before, downstream_before;
    lane_t downstream;

    function automatic row_t swapped(input row_t value);
        return '{value[0], value[1]};
    endfunction

    // Both arms of a structure conditional are dependencies even while one is
    // not selected; an unselected-arm change reruns the block without changing
    // its result (SV 9.2.2.2.1, 11.4.11).
    always_comb begin
        merged = choose_left ? left_record : right_record;
        merge_runs = merge_runs + 1;
    end

    // Rewriting `merged` with identical values is not an update event, so this
    // reader must not rerun for an unselected-arm change (SV 4.3, 9.2.2.2.1).
    always_comb begin
        downstream = merged.data;
        downstream_runs = downstream_runs + 1;
    end

    // An unpacked-array set member contributes its element contents.
    always_comb hit = key inside {table_row};

    // Storage compared with a structure parameter depends on every member.
    always_comb equal_reference = left_record === REFERENCE;

    // A later predicate clause reads `threshold` only after a tag match.
    always_comb begin
        matched = 8'h00;
        if (maybe matches tagged valid .payload &&& payload > threshold)
            matched = payload;
    end

    // Positional lvalue writes are excluded from the block's own sensitivity;
    // the swapped call keeps its argument dependency.
    always_comb '{first_lane, second_lane} = swapped(source_row);

    // Verilog-2001 implicit sensitivity sees the selector and the memory word.
    always @* star_word = memory[address];

    // A latch holds its record-member value while closed.
    always_latch if (latch_open) latched = left_record.data;

    // A whole-record nonblocking write changes only at the clock edge.
    always_ff @(posedge clk) registered <= merged;

    task automatic report(input string label);
        $display("%s merged=%h/%b/%h%h hit=%b eq=%b matched=%h lanes=%h,%h word=%h latch=%h reg=%h",
                 label, merged.data, merged.flag, merged.row[0], merged.row[1], hit,
                 equal_reference, matched, first_lane, second_lane, star_word, latched,
                 registered.data);
    endtask

    initial begin
        clk = 0;
        latch_open = 1;
        choose_left = 1;
        left_record = REFERENCE;
        right_record = '{data:8'h33, flag:0, row:'{4'h7, 4'h8}};
        table_row = '{8'h10, 8'h20};
        key = 8'h20;
        maybe = tagged valid 8'h40;
        threshold = 8'h30;
        source_row = '{8'hab, 8'hcd};
        memory = '{8'hd0, 8'hd1, 8'hd2, 8'hd3};
        address = 2'd1;
        #1 report("initial");

        // Unselected arm: the block reruns, the result is unchanged.
        runs_before = merge_runs;
        downstream_before = downstream_runs;
        right_record.row[1] = 4'hf;
        #1 report("unselected");
        if (merge_runs - runs_before != 1)
            $fatal(1, "unselected structure arm did not wake always_comb exactly once");
        if (downstream_runs != downstream_before)
            $fatal(1, "an unchanged structure result notified its reader");

        choose_left = 0;
        #1 report("selected");
        if (downstream_runs - downstream_before != 1 || downstream !== 8'h33)
            $fatal(1, "a changed structure result did not notify its reader once");
        left_record.data = 8'h5b;
        table_row[0] = 8'h11;
        #1 report("members");
        key = 8'h11;
        threshold = 8'h40;
        #1 report("predicate");
        threshold = 8'h3f;
        source_row[1] = 8'hee;
        memory[1] = 8'he1;
        #1 report("contents");
        address = 2'd3;
        latch_open = 0;
        #1 report("selector");
        left_record.data = 8'h77;
        #1 report("held");
        clk = 1;
        #1 report("edge");
        $finish(0);
    end
endmodule
