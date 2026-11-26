// A static class property of an unpacked tagged union (readme: "Unpacked
// properties").
typedef union tagged {
  int a;
  logic [7:0] b;
} choice_t;
class st;
  static choice_t ch;
endclass
module tb;
  st o = new;
  initial begin
    st::ch = tagged a 5;
    $display("a %0d", o.ch.a);
    case (o.ch) matches
      tagged a .v: $display("case a %0d", v);
      tagged b .v: $display("case b %h", v);
    endcase
    o.ch = tagged b 8'h77;
    st::ch.b[3:0] = 1;
    case (st::ch) matches
      tagged a .v: $display("case a %0d", v);
      tagged b .v: $display("case b %h", v);
    endcase
  end
endmodule
