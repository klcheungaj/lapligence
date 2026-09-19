// llg-test-fixture: G1-34 rtl_composition_gate.
// Multi-driver tri-state resolution feeding a resettable always_ff capture,
// explicit scalar drive strengths (IEEE 1800-2009 10.3.4 permits strength on
// scalar nets), and wired-AND / wired-OR nets with their four-state truth
// tables.
module tb;
    logic       va;
    logic       vb;
    logic       en_a;
    logic       en_b;
    tri         sbus;

    assign (strong0, strong1) sbus = en_a ? va : 1'bz;
    assign (weak0, weak1)     sbus = en_b ? vb : 1'bz;

    logic [3:0] v2a;
    logic [3:0] v2b;
    logic       en2a;
    logic       en2b;
    tri   [3:0] bus2;

    assign bus2 = en2a ? v2a : 4'hz;
    assign bus2 = en2b ? v2b : 4'hz;

    logic a;
    logic b;
    wand  wa;
    wor   wo;
    assign wa = a;
    assign wa = b;
    assign wo = a;
    assign wo = b;

    logic clk;
    logic rst_n;
    logic [3:0] cap;
    always_ff @(posedge clk) begin
        if (!rst_n) cap <= '0;
        else cap <= bus2;
    end

    initial begin
        clk = 0;
        rst_n = 0;
        va = 1'b1;
        vb = 1'b0;
        v2a = 4'h3;
        v2b = 4'hC;
        en_a = 1'b0;
        en_b = 1'b0;
        en2a = 1'b0;
        en2b = 1'b0;
        a = 1'b0;
        b = 1'b0;
        #1 $display("z sbus=%b bus2=%h wand=%b wor=%b", sbus, bus2, wa, wo);
        en_a = 1'b1;
        #1 $display("a sbus=%b", sbus);
        en_b = 1'b1;
        #1 $display("ab sbus=%b", sbus);
        va = 1'b0;
        #1 $display("strong sbus=%b", sbus);
        en_a = 1'b0;
        #1 $display("b sbus=%b", sbus);
        en2a = 1'b1;
        en2b = 1'b1;
        #1 $display("conflict bus2=%h", bus2);
        rst_n = 1'b1;
        en2a = 1'b1;
        en2b = 1'b0;
        v2a = 4'h5;
        #1 clk = 1;
        #1 clk = 0;
        $display("cap=%h bus2=%h", cap, bus2);
        a = 1'b1;
        b = 1'b0;
        #1 $display("wired1 wand=%b wor=%b", wa, wo);
        a = 1'b1;
        b = 1'b1;
        #1 $display("wired2 wand=%b wor=%b", wa, wo);
        a = 1'bx;
        b = 1'b1;
        #1 $display("wired3 wand=%b wor=%b", wa, wo);
        a = 1'bz;
        b = 1'b0;
        #1 $display("wired4 wand=%b wor=%b", wa, wo);
        $finish(0);
    end
endmodule
