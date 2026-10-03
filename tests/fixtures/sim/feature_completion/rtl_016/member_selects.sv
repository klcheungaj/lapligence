// IEEE 1800-2009 7.3.2, 11.5.1, 11.9: bit, part, indexed-part and element
// selects of an active tagged member read and write that member's storage
// only; constant and runtime selectors are evaluated once, and selectors
// outside the member read X and write nothing (11.5.1).
typedef union tagged packed {
    logic [7:0] A;
    struct packed { logic [3:0] hi; logic [3:0] lo; } S;
} packed_t;

typedef union tagged {
    void Idle;
    logic [7:0] Row [0:3];
    logic [3:0][3:0] Lanes;
} unpacked_t;

module tb;
    packed_t p;
    unpacked_t u, copy;
    int i, calls;
    logic [3:0] nibble;

    function automatic int pick(input int value);
        calls++;
        return value;
    endfunction

    initial begin
        calls = 0;
        p = tagged A (8'b1010_0101);
        $display("p bit=%b part=%b", p.A[7], p.A[3:0]);
        i = 2;
        $display("p runtime bit=%b indexed=%b down=%b", p.A[i], p.A[i +: 4], p.A[i+3 -: 4]);
        p.A[0] = 1'b0;
        p.A[pick(6) +: 2] = 2'b01;
        $display("p after=%b calls=%0d", p.A, calls);
        p.A[i] <= 1'b1;
        i = 7;
        #1 $display("p nba=%b", p.A);
        $display("p out_of_range=%b", p.A[i + 4]);
        p.A[i + 4] = 1'b0;
        $display("p unchanged=%b", p.A);
        p = tagged S '{hi: 4'h3, lo: 4'hc};
        p.S.lo[1] = 1'b1;
        $display("p S lo=%h hi_bit=%b", p.S.lo, p.S.hi[1]);

        u = tagged Row '{8'h00, 8'h11, 8'h22, 8'h33};
        $display("u row=%h %h %h %h", u.Row[0], u.Row[1], u.Row[2], u.Row[3]);
        i = 3;
        $display("u runtime=%h bit=%b out=%h", u.Row[i], u.Row[i][0], u.Row[i + 1]);
        u.Row[pick(1)] = 8'haa;
        u.Row[i][7:4] = 4'hf;
        u.Row[i + 1] = 8'hee;
        $display("u row=%h %h %h %h calls=%0d", u.Row[0], u.Row[1], u.Row[2], u.Row[3], calls);
        u.Row[i] <= 8'h5a;
        i = 0;
        #1 $display("u nba=%h %h", u.Row[0], u.Row[3]);
        copy = u;
        u.Row[2] = 8'h01;
        $display("copy=%h u=%h", copy.Row[2], u.Row[2]);
        u = tagged Lanes (16'h4321);
        $display("lanes=%h %h", u.Lanes[0], u.Lanes[3]);
        u.Lanes[1] = 4'h9;
        nibble = u.Lanes[i + 1];
        $display("lanes=%h nibble=%h", u.Lanes, nibble);
        $finish(0);
    end
endmodule
