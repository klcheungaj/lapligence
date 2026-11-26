// The operands of a nonblocking assignment to an element of an array of
// handles or strings are evaluated in the order of one to a packed element.
`timescale 1ns / 1ns

module tb;
  class c;
  endclass
  event ev;
  c handles[4], g = new;
  string strings[4];
  int values[4];

  function automatic int n(string name, int value);
    $write(" %s", name);
    return value;
  endfunction

  function automatic c obj();
    $write(" source");
    return g;
  endfunction

  function automatic string str();
    $write(" source");
    return "s";
  endfunction

  initial begin
    $write("packed");
    values[n("target", 0)] <= n("source", 1);
    $write(" |");
    values[n("target", 1)] <= #(n("delay", 1)) n("source", 1);
    $write(" |");
    values[n("target", 2)] <= @ev n("source", 1);
    $write(" |");
    values[n("target", 3)] <= repeat (n("count", 1)) @ev n("source", 1);
    $write("\nhandle");
    handles[n("target", 0)] <= obj();
    $write(" |");
    handles[n("target", 1)] <= #(n("delay", 1)) obj();
    $write(" |");
    handles[n("target", 2)] <= @ev obj();
    $write(" |");
    handles[n("target", 3)] <= repeat (n("count", 1)) @ev obj();
    $write("\nstring");
    strings[n("target", 0)] <= str();
    $write(" |");
    strings[n("target", 1)] <= #(n("delay", 1)) str();
    $write(" |");
    strings[n("target", 2)] <= @ev str();
    $write(" |");
    strings[n("target", 3)] <= repeat (n("count", 1)) @ev str();
    $write("\nrows  ");
    strings[0+:2] <= #(n("delay", 1)) strings[n("source", 2)+:2];
    $write(" |");
    strings[n("target", 0)+:2] <= @ev strings[n("source", 2)+:2];
    $write("\n");
    #2 -> ev;
    #1 $display("%p %b %p", values, handles[0] == g && handles[1] == g && handles[2] == g
                && handles[3] == g, strings);
    $finish;
  end
endmodule
