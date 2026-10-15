// 18.5.9 L30071-30072: circular solve...before orders are an error.
class c_c;
  rand bit [3:0] x, y;
  constraint a { solve x before y; }
  constraint b { solve y before x; }
endclass
module tb;
  initial begin
    static c_c c = new;
    int ok;
    ok = c.randomize();
    $display("ok=%0d", ok);
    c.b.constraint_mode(0);
    ok = c.randomize();
    $display("ok=%0d", ok);
    $finish;
  end
endmodule
