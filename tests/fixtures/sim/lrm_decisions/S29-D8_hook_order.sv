// IEEE 1800-2009 18.6.2 L30445-30447: "When obj.randomize() is invoked, it
// first invokes pre_randomize() on obj and also all of its random object
// members that are enabled. After the new random values are computed and
// assigned, randomize() invokes post_randomize() on obj".
// Decision (LRM text): pre_randomize() runs before any state value or mode is
// read, so state and rand_mode/constraint_mode changes it makes apply to this
// call; post_randomize() runs after the new values are assigned and only when
// the call succeeds (18.6.3 L30473).
class item_c;
  rand bit [7:0] a;
  bit [7:0] limit;
  bit [7:0] seen_in_post;
  int posts;
  constraint below { a < limit; }
  function void pre_randomize();
    limit = 8'd4;
    a.rand_mode(1);
  endfunction
  function void post_randomize();
    seen_in_post = a;
    posts++;
  endfunction
endclass
module tb;
  initial begin
    static item_c item = new;
    int ok;
    item.limit = 8'd0;
    item.a = 8'd200;
    item.a.rand_mode(0);
    ok = item.randomize();
    $display("ok=%0d below=%0d post_saw_new=%0d posts=%0d", ok, item.a < 8'd4,
             item.seen_in_post == item.a, item.posts);
    ok = item.randomize() with { a > 8'd10; };
    $display("failed ok=%0d posts=%0d", ok, item.posts);
    $finish;
  end
endmodule
