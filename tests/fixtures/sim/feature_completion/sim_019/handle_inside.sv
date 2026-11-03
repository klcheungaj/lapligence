// FIN-001 G3 (F07-11): `inside` over class handles. Membership is equality
// with each set item (SV 11.4.13) and handle equality compares object
// identity (SV 8.4), so variables, properties, array elements and `null`
// are set items; a null handle is a member of a set holding `null`.
module tb;
  class C;
    int id;
    C peer;
    function new(int i); id = i; endfunction
  endclass
  C a = new(1), b = new(2), h, k;
  C arr[2];
  int hits = 0;
  initial begin
    h = b;
    arr[0] = a; arr[1] = null;
    $display("A %0d %0d %0d", h inside {a, b}, a inside {b, null}, k inside {a, null});
    a.peer = b;
    $display("B %0d %0d", a.peer inside {h}, b.peer inside {arr[1]});
    if (h inside {arr[0]}) hits++;
    h = a;
    if (h inside {arr[0], b}) hits += 10;
    $display("C %0d %0d", hits, !(h inside {b}));
    $finish;
  end
endmodule
