// SV 11.3.5: a skipped operand makes no access, so a guarded property
// container of a null handle stops nothing.
module tb;
  class C; int q[$]; endclass
  C h;
  int v;
  initial begin
    if (h != null && h.q.size() > 0) v = 1;
    v = (h == null) ? 7 : h.q.size();
    $display("%0d", v);
    $finish;
  end
endmodule
