// Static-review counterexample; NOT EXECUTED.
class C;
  static int calls = 0;
  static function string make_label();
    calls++;
    return "label";
  endfunction
  string label = make_label();
endclass
module tb;
  C c;
  initial begin
    c = new;
    if (C::calls != 1) $fatal(1, "property initializer was silently omitted");
    $finish(0);
  end
endmodule
