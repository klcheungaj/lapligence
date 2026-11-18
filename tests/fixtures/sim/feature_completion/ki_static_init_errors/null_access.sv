// A null handle access in the constructor a class handle's declaration
// initializer calls: the run ends at the access. Expected values are derived
// in readme.md.
class leaf;
  int x = 3;
endclass
class holder;
  leaf l;
  int copy;
  function new();
    $display("holder new");
    copy = l.x;
    $display("holder copied %0d", copy);
  endfunction
endclass
module tb;
  function automatic int note(string s);
    $display("init %s", s);
    return 1;
  endfunction
  int first = note("first");
  holder h = new;
  int last = note("last");
  initial begin
    $display("initial first=%0d last=%0d", first, last);
    $finish;
  end
endmodule
