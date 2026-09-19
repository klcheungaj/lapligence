// llg-test-fixture: tests/sim_net_resolution.rs/truth_matrix.sv
// LRM: IEEE 1364-2001 6.1.2 / IEEE 1800-2009 6.6-6.7. Exhaustive two-driver
// table over {0,1,x,z} for a plain wire, wired-AND, wired-OR, and a
// strong-vs-weak strength pair. The expected output is hand-derived from the
// resolution tables, independent of the runtime resolver under test.
module tb;
    reg va, vb;
    wire w;
    wand wa;
    wor wo;
    wire sw;

    assign w = va;
    assign w = vb;
    assign wa = va;
    assign wa = vb;
    assign wo = va;
    assign wo = vb;
    assign (strong0, strong1) sw = va;
    assign (weak0, weak1) sw = vb;

    integer i, j;
    initial begin
        for (i = 0; i < 4; i = i + 1) begin
            for (j = 0; j < 4; j = j + 1) begin
                va = (i == 2) ? 1'bx : (i == 3) ? 1'bz : i[0];
                vb = (j == 2) ? 1'bx : (j == 3) ? 1'bz : j[0];
                #1;
                $display("%b %b | %b %b %b %b", va, vb, w, wa, wo, sw);
            end
        end
        $finish(0);
    end
endmodule
