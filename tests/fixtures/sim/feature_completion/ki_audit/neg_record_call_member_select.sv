// A member array of fewer than 16 cells selected on a call result: a located
// limit of the per-cell form (docs/known_issues.md, unpacked records with
// dense member arrays).
module tb;
  typedef struct { int a[4]; string s; } rec_t;
  function automatic rec_t f();
    f.a = '{1, 2, 3, 4};
    f.s = "f";
  endfunction
  initial begin
    int x;
    x = f().a[1];
    $display("%0d", x);
    $finish(0);
  end
endmodule
