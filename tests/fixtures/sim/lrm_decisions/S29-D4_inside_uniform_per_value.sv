// IEEE 1800-2009 18.5.3 L29617-29618: "Absent any other constraints, all
// values (either single values or value ranges) have an equal probability of
// being chosen by the inside operator." 18.5.9 L30013-30014: "the random
// values are selected to give a uniform value distribution over legal value
// combinations".
// Decision (llg reading of the two sentences together): every legal *value*
// is equally likely; a range is not weighted as one item. With
// `x inside {[0:99], 200}`, 200 is one value of 101, so it occurs in about
// 1 % of the draws, not the 50 % a per-item reading would give.
class pick_c;
  rand bit [7:0] x;
  constraint c { x inside {[8'd0:8'd99], 8'd200}; }
endclass
module tb;
  initial begin
    static pick_c pick = new;
    int hits, outside;
    hits = 0;
    outside = 0;
    repeat (10000) begin
      void'(pick.randomize());
      if (pick.x == 8'd200) hits++;
      else if (pick.x > 8'd99) outside++;
    end
    $display("outside=%0d rare_200=%0d", outside, hits < 500);
    $finish;
  end
endmodule
