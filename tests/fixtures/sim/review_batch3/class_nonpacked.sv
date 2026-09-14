// Static-review regression; not executed during patch preparation.
class Child;
  int value = 13;
endclass
class Box;
  static int calls = 0;
  string text = initialize_text();
  Child child = new;
  function string initialize_text();
    calls++;
    return "ready";
  endfunction
  function new();
    if (text != "ready" || child == null) $fatal(1, "nonpacked defaults");
  endfunction
endclass
module tb;
  Box a, b;
  initial begin
    a = new;
    b = new;
    if (a.text != "ready" || b.text != "ready" || a.child == b.child || Box::calls != 2)
      $fatal(1, "initializer ownership or evaluation count");
    $display("class nonpacked ok");
    $finish(0);
  end
endmodule
