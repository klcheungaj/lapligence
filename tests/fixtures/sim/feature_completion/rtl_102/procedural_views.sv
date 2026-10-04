// IEEE 1800-2009 25.5.4: procedural reads, writes, selections and event
// sensitivity through modport expression ports.
interface bus_if;
  logic [7:0] data;
  logic [0:7] asc;
  logic [3:0][7:0] words;
  logic [7:0] mem [0:3];
  logic a, c;
  logic [1:0] m;
  localparam logic [7:0] K = 8'd3;
  modport rd (input .sum(data[3:0] + data[7:4]), input .k(K), input .nib(data[3:0]),
              input .bit2(data[2]), input .ascp(asc[2:5]), input .el(mem[2]),
              input .wd(words));
  modport wr (output .hi(data[7:4]), output .ascw(asc[0:3]), output .wds(words),
              output .el(mem[1]), output .cat({a, m, c}));
endinterface
module reader(bus_if.rd b, input logic [1:0] i, output logic [7:0] s, output logic [3:0] y,
              output logic z, output logic [3:0] ap, output logic a3,
              output logic [7:0] el, output logic [7:0] sel, output logic [3:0] nib2);
  always_comb s = 8'(b.sum) + b.k;
  always_comb y = b.nib + 4'd1;
  always @(b.bit2) z = b.bit2;
  assign ap = b.ascp;
  assign a3 = b.ascp[3];
  assign el = b.el;
  assign sel = b.wd[i];
  assign nib2 = b.wd[2][7:4];
endmodule
module writer(bus_if.wr b, input logic clk, input logic [3:0] v, input logic [1:0] j);
  always @(posedge clk) begin
    b.hi <= v;
    b.cat <= v;
  end
  always @(negedge clk) begin
    b.ascw = 4'b1010;
    b.ascw[1] = 1'b1;
    b.wds[3] = 8'hbe;
    b.wds[2][3:0] = 4'h5;
    b.wds[j] = 8'h91;
    b.wds[j][0 +: 4] = 4'h3;
    b.hi[3'(j) + 3'd5] = 1'b0;
    b.el = 8'h77;
    b.cat[1] = 1'b0;
  end
endmodule
module tb;
  bus_if bi();
  logic clk = 0;
  logic [3:0] v, y, ap, nib2;
  logic [1:0] i, j;
  logic [7:0] s, el, sel;
  logic z, a3;
  reader r(bi, i, s, y, z, ap, a3, el, sel, nib2);
  writer w(bi, clk, v, j);
  initial begin
    bi.data = 8'h21; bi.asc = 8'b0011_1100; bi.words = {8'h44, 8'h33, 8'h22, 8'h11};
    bi.mem[1] = 8'h00; bi.mem[2] = 8'h5a; i = 2'd1; j = 2'd0; v = 4'hb;
    #1 $display("%h %h %b %b %h %h %h", s, y, ap, a3, el, sel, nib2);
    bi.data[3:0] = 4'h4; i = 2'd3;
    #1 $display("%h %h %b %h", s, y, z, sel);
    clk = 1;
    #1 $display("%h %b%b%b %h", bi.data, bi.a, bi.m, bi.c, s);
    clk = 0;
    #1 $display("%b %h %h %b%b%b %h %h %h", bi.asc, bi.words, bi.mem[1], bi.a, bi.m, bi.c,
                bi.data, sel, nib2);
    $finish(0);
  end
endmodule
