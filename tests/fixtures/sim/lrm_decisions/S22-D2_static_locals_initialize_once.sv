// Decision S22-D2: an explicitly static local of a subroutine or block,
// including a string, class handle, queue or a structure with such members,
// is initialized once before any procedure starts, not when its subroutine
// is first called. Its initializer therefore reads the values other
// declarations had before time 0 even when a procedure changes them before
// the first call, and each later call sees the value the previous call left.
// Automatic locals are initialized again on every entry.
//
// IEEE 1800-2009 6.21 (SystemVerilog-1800-2009.txt L6904-6905):
//   "Specific variables within an automatic task, function, or block can be
//   explicitly declared as static. Such variables have a static lifetime."
// IEEE 1800-2009 6.21 (L6942-6944):
//   "an explicit static keyword shall be required when an initialization
//   value is specified as part of a static variable's declaration to indicate
//   the user's intent of executing that initialization only once at the
//   beginning of simulation."
// IEEE 1800-2009 6.21 (L6900-6901): variables of an automatic subroutine
//   "default to the lifetime of the call or block and are initialized on each
//   entry to the call or block".
module tb;
  class Item;
    int n;
    function new(int v);
      n = v;
    endfunction
  endclass
  typedef struct { string s; int q[$]; } rec_t;

  string base = "orig";

  function automatic string step(int k);
    static string snap = base;
    static Item item = new(10);
    static int q[$] = '{1, 2};
    static rec_t r = '{s: "r", q: '{7}};
    string fresh = "a";
    item.n += k;
    q.push_back(k);
    r.s = {r.s, "+"};
    r.q.push_back(k);
    fresh = {fresh, "b"};
    return $sformatf("snap=%s item=%0d q=%0d r=%s/%0d fresh=%s",
                     snap, item.n, q.size(), r.s, r.q.size(), fresh);
  endfunction

  task automatic tally();
    static string seen = "t";
    string local_text = "x";
    seen = {seen, "t"};
    local_text = {local_text, "x"};
    $display("tally seen=%s local=%s", seen, local_text);
  endtask

  initial begin
    base = "changed";
    $display("%s", step(1));
    $display("%s", step(2));
    tally();
    tally();
    for (int i = 0; i < 2; i++) begin
      static string kept = "k";
      automatic string again = "g";
      kept = {kept, "k"};
      again = {again, "g"};
      $display("loop kept=%s again=%s", kept, again);
    end
    $finish;
  end
endmodule
