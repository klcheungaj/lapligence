// SV2009 6.22.2, 11.4.5: unpacked structures compare only when equivalent;
// identical member lists in distinct declarations are not equivalent.
module tb;
  typedef struct { logic [3:0] a; bit [1:0] b; } left_t;
  typedef struct { logic [3:0] a; bit [1:0] b; } right_t;
  left_t l;
  right_t r;
  initial begin
    $display("%b", l == r);
    $finish(0);
  end
endmodule
