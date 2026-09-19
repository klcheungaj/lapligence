// llg-test-fixture: G1-34 rtl_composition_gate (IEEE 1800-2009 scope).
// The 2009 counterpart of edition_scope_2001: same datapath and expected trace,
// using SystemVerilog logic, always_comb/always_ff, an inline generate loop and
// a typed automatic function. Run with `--edition 2009`.
module tb;
    parameter int W = 8;

    function automatic logic [W-1:0] mix(input logic [W-1:0] a,
                                         input logic [W-1:0] b);
        mix = (a & b) | (a ^ b);
    endfunction

    logic [W-1:0] va;
    logic [W-1:0] vb;
    logic [W-1:0] combined;
    assign combined = mix(va, vb);

    logic [W-1:0] lanes [0:3];
    for (genvar i = 0; i < 4; i++) begin : g
        always_comb lanes[i] = combined + i[W-1:0];
    end

    logic clk;
    logic rst_n;
    logic [W+1:0] acc;
    always_ff @(posedge clk) begin
        if (!rst_n) acc <= '0;
        else acc <= lanes[0] + lanes[1] + lanes[2] + lanes[3];
    end

    initial begin
        clk = 0;
        rst_n = 0;
        va = 8'hF0;
        vb = 8'h3C;
        #1 clk = 1;
        #1 clk = 0;
        rst_n = 1;
        #1 clk = 1;
        #1 clk = 0;
        $display("combined=%h acc=%h lanes=%h %h %h %h",
                 combined, acc, lanes[0], lanes[1], lanes[2], lanes[3]);
        $finish(0);
    end
endmodule
