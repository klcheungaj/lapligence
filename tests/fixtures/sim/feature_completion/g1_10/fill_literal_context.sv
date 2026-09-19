// IEEE 1800-2009 5.7.1, 11.4.12, 11.4.14: an unbased unsized fill literal
// takes the width of its immediate assignment/expression context and does not
// disturb adjacent packed members that share the enclosing storage. The
// expected trace is an independent LRM-derived constant in tests/sim_g1_closure.rs.
module tb;
    typedef struct packed { logic [3:0] hi; logic [3:0] lo; } pair_t;

    pair_t p;
    logic [15:0] word;
    logic [7:0] eight;
    logic [1:0][7:0] lanes;
    logic [3:0] nib;
    logic cond;

    function automatic [15:0] take16(input [15:0] value);
        take16 = value;
    endfunction

    initial begin
        p = 8'h00;
        p.lo = '1;
        $display("member hi=%b lo=%b whole=%b", p.hi, p.lo, p);
        p.hi = '0;
        $display("isolate hi=%b lo=%b", p.hi, p.lo);

        word = 16'h0000;
        word[11:8] = '1;
        $display("part high=%b low=%b", word[15:12], word[7:0]);

        lanes = '0;
        lanes[1] = '1;
        $display("lanes0=%b lanes1=%b", lanes[0], lanes[1]);

        eight = 'x;
        $display("eight=%b", eight);

        nib = '0;
        nib = nib | '1;
        $display("or=%b", nib);

        cond = 1'b1;
        $display("ternary=%b", cond ? '1 : 8'h00);
        $display("fn=%b", take16('1 & 16'h33cc));
        $display("concat=%b", {'1, 4'hA, '0});
        $display("bits=%0d", $bits('z));

        $display("PASS fill_literal_context");
        $finish(0);
    end
endmodule
