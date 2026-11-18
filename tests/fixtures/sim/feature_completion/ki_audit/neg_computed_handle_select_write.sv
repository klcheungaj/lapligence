// A packed select of a computed handle's property as a write target: a
// located limit (docs/known_issues.md, selects of properties of computed
// class handles).
module tb;
  class C;
    bit [7:0] b;
  endclass
  C h = new;
  function automatic C get();
    return h;
  endfunction
  initial begin
    get().b[2] = 1'b1;
    $display("%b", h.b);
    $finish;
  end
endmodule
