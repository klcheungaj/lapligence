// llg-test-fixture: tests/fixtures/sim/sv4_gmp_integration/activation_owners.sv
// Packed locals, formals and results of recursive subprograms on the chain
// arena, wide waits canceled by disable fork, and arithmetic destinations
// aliasing their operands (IEEE 1800-2009 §13.4.2, §9.6.3, §11.4).
`timescale 1ns/1ns
module tb;
    logic [199:0] watched;
    logic [199:0] acc;
    int failures = 0;
    int woke = 0;

    // fib(n) as a 200-bit value; each frame keeps two wide locals live
    // across its recursive calls.
    function automatic logic [199:0] fib(input int n, input logic [199:0] bias);
        logic [199:0] left, right;
        if (n < 2) return n + bias - bias;
        left = fib(n - 1, bias + 200'd1);
        right = fib(n - 2, {bias[0 +: 100], bias[100 +: 100]});
        return left + right;
    endfunction

    // Wide product through recursion; the result width exceeds 128 bits.
    function automatic logic [199:0] fact(input logic [199:0] n);
        if (n <= 1) return 200'd1;
        return n * fact(n - 1);
    endfunction

    task automatic wait_change();
        logic [199:0] seen;
        seen = watched;
        @(watched);
        woke++;
        if (watched === seen) failures++;
    endtask

    initial begin
        if (fib(20, {200{1'b1}}) !== 200'd6765) begin
            failures++;
            $display("FAIL fib %0d", fib(20, {200{1'b1}}));
        end
        if (fact(40) !== 200'd815915283247897734345611269596115894272000000000) begin
            failures++;
            $display("FAIL fact %0d", fact(40));
        end

        // Destination aliases both operands.
        acc = 200'h9312_3456_789a_bcde_f00f_edcb_a987_6543_2113_579b_df24_68ac_e0;
        acc = acc + acc;
        acc = acc * acc;
        acc = acc - acc[199:100];
        if (acc !== 200'h7a_7ad9_83cd_8600_016a_6cee_f17d_7122_b45a_fc28_2699_0428_20e8) begin
            failures++;
            $display("FAIL alias %h", acc);
        end

        watched = {200{1'b0}};
        fork
            wait_change();
            wait_change();
        join_none
        #1;
        disable fork;
        watched = {200{1'bx}};
        #1;
        fork
            wait_change();
        join_none
        #1;
        watched[150] = 1'b1;
        #1;
        if (woke != 1) begin
            failures++;
            $display("FAIL woke %0d", woke);
        end

        if (failures == 0) $display("PASS activation_owners");
        $finish(0);
    end
endmodule
