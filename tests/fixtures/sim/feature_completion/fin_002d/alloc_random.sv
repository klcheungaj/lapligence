// Allocation failure matrix: constrained randomization (the solver's
// plans, layers and scratch), random stability calls and weighted choices.
`timescale 1ns / 1ns
module tb;
  typedef enum {
    RED,
    GREEN,
    BLUE
  } colour_t;

  class frame;
    rand bit [7:0] length;
    rand bit [15:0] address;
    rand colour_t colour;
    rand bit [7:0] payload[];
    rand bit [3:0] slots[4];
    randc bit [2:0] turn;
    bit [7:0] limit = 8'd20;
    constraint size {
      length inside {[8'd4 : limit]};
      payload.size() == int'(length);
    }
    constraint order {
      solve length before address;
      address[3:0] == length[3:0];
      address dist {
        [16'h0000 : 16'h00ff] := 3,
        [16'h8000 : 16'hffff] :/ 1
      };
    }
    constraint shape {
      colour != GREEN -> length < 12;
      slots[0] != slots[1];
      slots[2] > slots[3];
      foreach (payload[i]) payload[i] < 8'd200;
    }
  endclass

  frame f = new;
  int sums[3];
  int failures;
  bit [31:0] word;
  bit [7:0] seen_turns;

  initial begin
    process::self().srandom(11);
    repeat (12) begin
      if (!f.randomize()) failures++;
      if (f.payload.size() != int'(f.length) || f.address[3:0] != f.length[3:0]) failures++;
      if (f.colour != GREEN && f.length >= 12) failures++;
      sums[f.colour] += 1;
      seen_turns[f.turn] = 1'b1;
    end
    if (f.slots[0] == f.slots[1] || f.slots[2] <= f.slots[3]) failures++;
    $display("frames %0d %0d %0d", sums[0] + sums[1] + sums[2], failures, $countones(seen_turns));
    if (!f.randomize() with {length == 6; colour == BLUE;}) failures++;
    $display("with %0d %0d %0d", f.length, f.payload.size(), f.colour == BLUE);
    f.limit = 8'd3;
    if (f.randomize()) failures++;
    $display("infeasible %0d", failures);
    f.limit = 8'd20;
    f.size.constraint_mode(0);
    f.length.rand_mode(0);
    void'(f.randomize());
    $display("modes %0d", f.length);
    if (!std::randomize(word) with {word[31:28] == 4'h5; word[3:0] inside {1, 2, 3};}) failures++;
    $display("scope %h %0d", word[31:28], word[3:0] < 4);
    repeat (5) begin
      randcase
        3: sums[0]++;
        1: sums[1]++;
      endcase
      word = $urandom_range(100, 50) + $urandom % 2;
      if (word < 50 || word > 101) failures++;
    end
    randsequence(main)
      main : first second;
      first : {sums[2] += 10;};
      second : {sums[2] += 100;} | {sums[2] += 200;};
    endsequence
    $display("choices %0d %0d", sums[0] + sums[1], failures);
    $display("done");
    $finish;
  end
endmodule
