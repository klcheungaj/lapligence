typedef struct packed { byte a; shortint b; } s_t;
class P;
  function void pk(int v, int size);
    int a[];
    begin
      bit [47:0] vec = 48'h0123_4567_89ab + 48'(v);
      { << int { a }} = {{($bits(int) - (48 % $bits(int))) {1'b0}}, vec};
    end
    foreach (a[i]) $display("%0d %h", i, a[i]);
  endfunction
endclass
module tb;
  P p = new;
  byte b;
  s_t s;
  function automatic logic [31:0] f();
    bit [3:0] l;
    l = 4'hf;
    return {{($bits(int) - $bits(l)) {1'b0}}, l};
  endfunction
  initial begin
    p.pk(0, 48);
    $display("%h", {{($bits(byte) - (3 % $bits(b))) {1'b1}}, 3'b000});
    $display("%h", {{($bits(s) - 20) {1'b1}}, 4'h0});
    $display("%h", {{($bits(s_t) / 8) {4'ha}}});
    $display("%h", f());
    $display("%h", {{($bits(int) - 32) {1'b1}}, 4'h5});
    $finish;
  end
endmodule
