// Sizes are solved before elements (S30-D5): the elements of an array cannot
// be solved before an array size.
class c_c;
  rand bit [3:0] a [];
  rand bit [3:0] f [2];
  constraint c { a.size() inside {[1:3]}; f[0] < a.size(); solve f before a.size(); }
endclass
module tb;
  initial begin
    static c_c c = new;
    void'(c.randomize());
    $finish;
  end
endmodule
