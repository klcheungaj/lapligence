// `$monitor` through a computed handle: nothing can name the storage without
// calling the function again, so it is a located limit (docs/known_issues.md).
module tb;
  class C; int v; endclass
  C c;
  function C f(); return c; endfunction
  initial begin
    c = new;
    $monitor("%0d", f().v);
    #1 c.v = 1;
    #1 $finish;
  end
endmodule
