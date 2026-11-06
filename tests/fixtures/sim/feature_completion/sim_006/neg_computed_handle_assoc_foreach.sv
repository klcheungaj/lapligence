// Each bound check of a keyed traversal would have to call `get` again.
module tb;
  class C;
    int m[string];
  endclass
  C h;
  function automatic C get();
    return h;
  endfunction
  initial begin
    h = new;
    foreach (get().m[k]) $display("%s", k);
  end
endmodule
