// llg-test-fixture: 18 structural drivers per net, above the retired 16-slot
// ceiling. Wired-AND resolves the single 0, wired-OR the single 1, and the
// plain wire reports the equal-strength conflict as X.
module tb;
    wire w;
    wand wa;
    wor wo;

    genvar i;
    generate
        for (i = 0; i < 17; i = i + 1) begin : g
            assign w = 1'b1;
            assign wa = 1'b1;
            assign wo = 1'b0;
        end
    endgenerate
    assign w = 1'b0;
    assign wa = 1'b0;
    assign wo = 1'b1;

    initial begin
        #1;
        $display("CHECK: w=%b wa=%b wo=%b", w, wa, wo);
        $finish(0);
    end
endmodule
