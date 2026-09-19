// llg-test-fixture: G1-34 rtl_composition_gate (strict IEEE 1364-2001 scope).
// The 2001 counterpart of edition_scope_2009: same datapath and expected trace,
// using only IEEE 1364-2001 constructs (reg/wire, always @*, generate for,
// old-style function). Run with `--edition 2001`.
module tb;
    parameter W = 8;

    function [W-1:0] mix;
        input [W-1:0] a;
        input [W-1:0] b;
        begin
            mix = (a & b) | (a ^ b);
        end
    endfunction

    reg  [W-1:0] va;
    reg  [W-1:0] vb;
    wire [W-1:0] combined;
    assign combined = mix(va, vb);

    reg [W-1:0] lanes [0:3];
    genvar i;
    generate
        for (i = 0; i < 4; i = i + 1) begin : g
            always @* lanes[i] = combined + i[W-1:0];
        end
    endgenerate

    reg clk;
    reg rst_n;
    reg [W+1:0] acc;
    always @(posedge clk) begin
        if (!rst_n) acc <= 0;
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
