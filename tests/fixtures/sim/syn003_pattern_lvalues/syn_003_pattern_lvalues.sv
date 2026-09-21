// llg-test-fixture: SYN-003 positional assignment-pattern lvalues
// LRM: IEEE 1800-2009 10.9 (assignment patterns).
module tb;
  typedef logic [7:0] U [0:1];
  typedef logic [7:0] R [1:0];

  logic [7:0] a, b;
  logic [7:0] c, d;
  logic [7:0] e, f, g, h;
  logic [7:0] nba_a, nba_b;
  U array_value;
  R reversed;
  logic [7:0] selected [0:3];
  logic [7:0] overlap [0:1];
  logic [7:0] nested [0:1][1:0];

  initial begin
    array_value[0] = 8'h11;
    array_value[1] = 8'h22;
    '{a, b} = array_value;
    $display("plain %h %h", a, b);

    U'{b, a} = array_value;
    $display("typed %h %h", a, b);

    '{a, b} = U'{8'h23, 8'h24};
    $display("pattern_rhs %h %h", a, b);

    '{selected[3], selected[1]} = array_value;
    $display("selected %h %h", selected[3], selected[1]);

    reversed[1] = 8'h31;
    reversed[0] = 8'h42;
    '{c, d} = reversed;
    $display("reversed %h %h", c, d);

    nested[0][1] = 8'h51;
    nested[0][0] = 8'h52;
    nested[1][1] = 8'h61;
    nested[1][0] = 8'h62;
    '{'{e, f}, '{g, h}} = nested;
    $display("nested %h %h %h %h", e, f, g, h);

    overlap[0] = 8'h71;
    overlap[1] = 8'h82;
    '{overlap[1], overlap[1]} = overlap;
    $display("overlap %h", overlap[1]);

    nba_a = 8'h00;
    nba_b = 8'h00;
    '{nba_a, nba_b} <= array_value;
    $display("nba_before %h %h", nba_a, nba_b);
    #1;
    $display("nba_after %h %h", nba_a, nba_b);
    $display("PASS syn_003_pattern_lvalues");
    $finish;
  end
endmodule
