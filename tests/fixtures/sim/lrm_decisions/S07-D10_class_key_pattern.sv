// S07-D10: `%p` prints a class key as `%p` prints that handle: the object's
// pattern, or `null`, never an address.
// 21.2.1.7 L36425-36426: "The %p and %0p format specifiers can also be used to
// print singular expressions, in which case the expression is formatted as an
// element of an aggregate expression described above."
class item_c;
  int id;
  function new(int i);
    id = i;
  endfunction
endclass

module tb;
  int table_a[item_c];
  string names[item_c];
  int nested[item_c][string];

  initial begin
    item_c first = new(1);
    item_c second = new(2);
    table_a[first] = 5;
    table_a[second] = 6;
    table_a[null] = 7;
    names[first] = "x";
    nested[second]["q"] = 1;
    $display("%p", first);
    $display("%p", table_a);
    $display("%p", names);
    $display("%p", nested);
    $finish;
  end
endmodule
