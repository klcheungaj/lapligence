// S13-D2: binding a `ref` or `const ref` formal to a missing associative
// array element creates the entry when the call starts, as a write would,
// whether or not the subroutine accesses the formal.
//
// 13.5.2 L19138-19141: "When the argument is passed by reference, both the
// caller and the subroutine share the same representation of the argument";
// 13.5.2 L19151-19154 lists an element of an unpacked array as a legal
// reference, and the shared representation of a missing element has to exist
// for the call. 7.8.6 only describes reads and writes, and the clause leaves
// the binding itself open. llg follows industry-standard behaviour: the entry
// exists from the bind on, and only its delete() outdates the reference
// (13.5.2 L19157-19168).
module tb;
  int a[int];
  string s[string];
  task automatic rd(const ref int x);
    $display("rd %0d %0d", x, a.num());
  endtask
  task automatic untouched(ref string x);
    $display("untouched %0d", s.num());
  endtask
  task automatic outdated(ref int x);
    x = 3;
    a.delete(6);
    x = 4;
    $display("outdated %0d %0d", x, a.num());
    a[6] = 8;
    x = 9;
    $display("kept %0d %0d", x, a[6]);
  endtask
  initial begin
    rd(a[5]);
    $display("%0d %0d", a.num(), a.exists(5));
    untouched(s["k"]);
    $display("%0d %0d", s.num(), s.exists("k"));
    outdated(a[6]);
    $finish;
  end
endmodule
