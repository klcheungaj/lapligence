// Outside actuals of every lvalue form on whole ports: concatenations of
// variables and nets, part-selects, indexed part-selects and bit-selects;
// plus the 23.2.2.1 examples that tie two ports to one inner net.
module o(output logic [3:0] y);
  initial begin y = 4'b1010; #2 y = 4'b0101; end
endmodule

module i(input [3:0] x);
endmodule

module io(inout [3:0] z);
  logic en;
  assign z = en ? 4'b1100 : 4'bzzzz;
  initial begin en = 1; #2 en = 0; end
endmodule

module same_input(a, a);
  input a;
endmodule

module same_port(.a(i), .b(i));
  inout i;
endmodule

module inv(input a, output y);
  assign y = ~a;
endmodule

module tb;
  logic [1:0] va, vb;
  wire [1:0] na, nb;
  logic [7:0] vw;
  wire [7:0] nw;
  wire [7:0] tw;
  logic [3:0] src;
  wire x, y;
  wire [3:0] n;
  logic [3:0] v;
  o c1(.y({va, vb}));
  o c2(.y({na, nb}));
  o c3(.y(vw[5:2]));
  o c4(.y(nw[3:0]));
  o c5(.y(nw[4+:4]));
  i c6(.x({src[1:0], src[3:2]}));
  io c8(.z(tw[7:4]));
  io c9(.z({tw[1:0], tw[3:2]}));
  same_input s1(1'b1, 1'b1);
  assign x = 1'b0;
  same_port s2(.a(x), .b(y));
  assign n[0] = 1'b0;
  inv g0(.a(n[0]), .y(n[1]));
  inv g1(.a(n[1]), .y(n[2]));
  inv g2(n[2], v[3]);
  initial begin
    src = 4'b0011;
    #1 $display("1 va=%b vb=%b na=%b nb=%b vw=%b nw=%b tw=%b x6=%b",
                va, vb, na, nb, vw, nw, tw, c6.x);
    $display("1 s1=%b x=%b y=%b n=%b v=%b", s1.a, x, y, n, v);
    #2 $display("3 va=%b vb=%b na=%b nb=%b vw=%b nw=%b tw=%b",
                va, vb, na, nb, vw, nw, tw);
    $finish;
  end
endmodule
