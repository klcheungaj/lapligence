// llg-test-fixture: tests/fixtures/sim/syn014_process_contexts/process_contexts.sv
// IEEE 1364-2001 §9.7.5 and §9.9.2; IEEE 1800-2009 §§7.2, 7.4.2, 7.4.6,
// 7.7, and 9.2.2.2–9.2.2.4: aggregate reads, called-function dependencies, disjoint
// packed writers, and always-family timing contracts are retained together.

typedef struct packed {
    logic [3:0] hi;
    logic [3:0] lo;
} pair_t;

typedef struct {
    pair_t pair;
    logic [7:0] lanes [0:1];
} record_t;

module aggregate_reader(input record_t source, output logic [7:0] sum);
    function automatic logic [7:0] fold(input record_t value);
        fold = value.lanes[0] + value.lanes[1];
    endfunction

    always_comb sum = fold(source);
endmodule

module tb;
    record_t source;
    logic select;
    logic latch_enable;
    logic clk;
    logic reset_n;
    logic [3:0] arm_value;
    logic [3:0] latched_value;
    logic [7:0] linked_sum;
    logic [7:0] registered_value;
    logic [7:0] split_value;
    pair_t feedback;
    logic [3:0] at_star_value;
    integer arm_calls = 0;
    integer before_calls;

    aggregate_reader reader(source, linked_sum);

    function automatic logic [3:0] choose_arm(input logic which);
        arm_calls = arm_calls + 1;
        if (which)
            choose_arm = source.pair.hi;
        else
            choose_arm = source.pair.lo;
    endfunction

    function automatic logic [3:0] hidden_source(input logic trigger);
        hidden_source = source.pair.hi;
        if (!trigger)
            hidden_source = source.pair.hi;
    endfunction

    // always_comb follows both aggregate leaves read by the helper, including
    // the arm that is not selected at the time of the update.
    always_comb arm_value = choose_arm(select);

    // The plain Verilog wildcard only sees the call-site argument. Changing
    // source alone must not wake this process; changing select does.
    always @* at_star_value = hidden_source(select);

    // Disjoint packed projections are independent procedural writers.
    always_comb split_value[3:0] = source.pair.lo;
    always_comb split_value[7:4] = source.pair.hi;

    // A read of a member written by the same always_comb is excluded from its
    // sensitivity set without excluding the other aggregate dependencies.
    always_comb begin
        feedback.hi = source.pair.hi;
        feedback.lo = feedback.hi;
    end

    // always_latch runs once at time zero and then retains its value while the
    // enable is low.
    always_latch if (latch_enable) latched_value = source.pair.lo;

    // always_ff is driven only by its declared reset/clock event control.
    always_ff @(posedge clk or negedge reset_n) begin
        if (!reset_n)
            registered_value <= 8'h00;
        else
            registered_value <= source.lanes[0];
    end

    initial begin
        source = '{pair:8'h21, lanes:'{8'h03, 8'h04}};
        select = 1'b0;
        latch_enable = 1'b0;
        clk = 1'b0;
        reset_n = 1'b0;
        #1;
        before_calls = arm_calls;
        // hi is the unselected helper arm. A change here must still wake the
        // process and call the helper again.
        source.pair.hi = 4'hc;
        #1;
        $display("aggregate arm=%h at=%h feedback=%h linked=%h split=%h latch=%h ff=%h delta=%0d",
                 arm_value, at_star_value, feedback.lo, linked_sum, split_value,
                 latched_value, registered_value, arm_calls - before_calls);

        reset_n = 1'b1;
        latch_enable = 1'b1;
        source.pair.lo = 4'h5;
        source.lanes[0] = 8'h11;
        #1;
        $display("enabled arm=%h at=%h feedback=%h linked=%h split=%h latch=%h ff=%h",
                 arm_value, at_star_value, feedback.lo, linked_sum, split_value,
                 latched_value, registered_value);

        select = 1'b1;
        #1;
        $display("triggered arm=%h at=%h feedback=%h linked=%h split=%h latch=%h ff=%h",
                 arm_value, at_star_value, feedback.lo, linked_sum, split_value,
                 latched_value, registered_value);

        // The aggregate input link and its child always_comb must observe a
        // nested array element update.
        source.lanes[1] = 8'h20;
        #1;
        $display("array arm=%h at=%h feedback=%h linked=%h split=%h latch=%h ff=%h",
                 arm_value, at_star_value, feedback.lo, linked_sum, split_value,
                 latched_value, registered_value);

        // The flip-flop does not respond to a data-only change until the next
        // declared edge, then captures the aggregate leaf present at that edge.
        source.lanes[0] = 8'h2a;
        #1;
        $display("held ff=%h", registered_value);
        clk = 1'b1;
        #1;
        $display("edge ff=%h", registered_value);

        reset_n = 1'b0;
        #1;
        $display("reset ff=%h", registered_value);
        $finish(0);
    end
endmodule
