// Decision PX-D2: an indexed part-select with a constant base (`+:`, `-:`)
// is a constant select, so it is a legal port reference in a module header.
//
// IEEE 1800-2009 23.2.2.1 (SystemVerilog-1800-2009.txt L40425-40426):
//   "port_reference ::=
//        port_identifier constant_select"
// IEEE 1800-2009 A.8.4 (L69126-69128) and A.8.3 (L69007-69013):
//   "constant_select ::=
//        [ { . member_identifier constant_bit_select } . member_identifier ] constant_bit_select
//            [ [ constant_part_select_range ] ]"
//   "constant_part_select_range ::=
//        constant_range
//      | constant_indexed_range"
//   "constant_indexed_range ::=
//        constant_expression +: constant_expression
//      | constant_expression -: constant_expression"
module mo(.p(a[0+:2]), .q(a[3-:2]));
  output logic [3:0] a;
  initial a = 4'b1001;
endmodule

module mi(.p(n[2+:2]), .q(n[1-:2]));
  input [3:0] n;
endmodule

module tb;
  wire [1:0] w1, w2;
  mo c1(.p(w1), .q(w2));
  mi c2(.p(2'b10), .q(2'b01));
  initial #1 begin
    $display("w1=%b w2=%b n=%b", w1, w2, c2.n);
    $finish;
  end
endmodule
