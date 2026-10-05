// RTL-101: a tagged union wider than the packed limit moves as a value.
module tb;
  typedef union tagged {
    logic [1023:0] w [0:2047];
    bit [7:0] t;
    void none;
  } tu_t;
  tu_t v, u;
  function automatic tu_t bump(input tu_t x);
    tu_t y;
    y = x;
    if (y matches tagged t .n) y = tagged t (n + 8'd1);
    return y;
  endfunction
  initial begin
    v = tagged t 8'd4;
    u = v;
    $display("A %0d %0d", u.t, u == v);
    u = tagged w '{default: 1024'd7};
    $display("B %0d %0d %0d", u.w[5][7:0], u == v, u != v);
    u.w[6] = 1024'd8;
    $display("C %0d %0d", u.w[6][7:0], u.w[5][7:0]);
    v = bump(v);
    $display("D %0d", v.t);
    case (u) matches
      tagged t .n: $display("E t %0d", n);
      tagged w .*: $display("E w");
      default: $display("E none");
    endcase
    if (v matches tagged t .k &&& k > 8'd2) $display("F %0d", k);
    v = tagged none;
    case (v) matches
      tagged t .n: $display("G t %0d", n);
      tagged none: $display("G none");
    endcase
    $finish(0);
  end
endmodule
