// IEEE 1800-2009 9.2.2.2-9.2.2.4, 23.3.3.3 and 27: generated instances of a
// parameterized stage each own one cell of a shared array through a ref port
// (always_ff) and one member of a shared record array (always_latch), while
// always_comb reductions and a called function read the composite at 4 and
// 65 bits. Every writer owns a disjoint cell or member.
module stage #(parameter int W = 4, parameter int K = 0)
    (ref logic [W-1:0] slot, ref logic [W-1:0] lat, input logic clk, input logic en,
     input logic [W-1:0] d);
    always_ff @(posedge clk) slot <= d + W'(K);
    always_latch if (en) lat = d ^ W'(K);
endmodule

module lane #(parameter int W = 4) (input logic clk, input logic en, input logic [W-1:0] d,
                                    output logic [W-1:0] sum, output logic [W-1:0] lsum);
    typedef struct { logic [W-1:0] l; logic [W-1:0] pad; } rec_t;
    logic [W-1:0] cells [0:3];
    rec_t recs [0:3];
    function automatic logic [W-1:0] total();
        logic [W-1:0] t = '0;
        for (int k = 0; k < 4; k++) t += cells[k];
        return t;
    endfunction
    for (genvar g = 0; g < 4; g++) begin : gen
        stage #(.W(W), .K(g)) s(.slot(cells[g]), .lat(recs[g].l), .clk(clk), .en(en), .d(d));
        always_comb recs[g].pad = W'(g) | (d & W'(0));
    end
    always_comb sum = total();
    always_comb begin
        lsum = '0;
        for (int k = 0; k < 4; k++) lsum += recs[k].l + recs[k].pad;
    end
endmodule

module tb;
    logic clk, en;
    logic [3:0] d4, s4, l4;
    logic [64:0] d65, s65, l65;
    lane #(.W(4)) narrow(.clk(clk), .en(en), .d(d4), .sum(s4), .lsum(l4));
    lane #(.W(65)) wide(.clk(clk), .en(en), .d(d65), .sum(s65), .lsum(l65));
    initial begin
        clk = 0;
        en = 1;
        d4 = 4'h3;
        d65 = {1'b1, 64'h0000_0000_0000_0010};
        #1 $display("t1 %h %h %h %h", s4, l4, s65, l65);
        clk = 1;
        #1 $display("t2 %h %h %h %h", s4, l4, s65, l65);
        clk = 0;
        en = 0;
        d4 = 4'h8;
        d65 = 65'h1;
        #1 $display("t3 %h %h %h %h", s4, l4, s65, l65);
        clk = 1;
        #1 $display("t4 %h %h %h %h", s4, l4, s65, l65);
        $finish(0);
    end
endmodule
