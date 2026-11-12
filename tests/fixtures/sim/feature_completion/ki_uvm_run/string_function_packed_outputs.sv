// A string-returning function with packed output and inout formals, called
// in expressions: copy-out at the return, static and automatic formals.
module tb;
  function automatic string index_of(string arg, output bit wild);
    wild = 1;
    for (int i = 0; i < arg.len(); i++)
      if (arg[i] == "[") begin
        wild = 0;
        return arg.substr(i + 1, arg.len() - 2);
      end
    return "";
  endfunction

  // Static function: its formals keep their storage between calls.
  function string tally(inout int count, output logic [3:0] last, input string s);
    count += s.len();
    last = 4'(unsigned'(s.len()));
    tally = $sformatf("%s:%0d", s, count);
  endfunction

  class holder;
    bit w;
    logic [3:0] last;
    int n;
    function string get(string s);
      return {index_of(s, w), "/", tally(n, last, s)};
    endfunction
  endclass

  initial begin
    static bit w = 1;
    static int count = 10;
    static logic [3:0] last = 'x;
    static holder h = new;
    string r;
    r = index_of("abc[12]", w);
    $display("1: %s %0d", r, w);
    r = {"<", index_of("abc", w), ">"};
    $display("2: %s %0d", r, w);
    r = tally(count, last, "four");
    $display("3: %s %0d %0d", r, count, last);
    r = {tally(count, last, "xy"), "!"};
    $display("4: %s %0d %0d", r, count, last);
    r = h.get("q[7]");
    $display("5: %s %0d %0d %0d", r, h.w, h.n, h.last);
    r = h.get("plain");
    $display("6: %s %0d %0d %0d", r, h.w, h.n, h.last);
    $finish;
  end
endmodule
