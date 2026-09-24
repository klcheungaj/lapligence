// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/process_write_remainders.sv
module tb;
  typedef logic [7:0] byte_t;

  logic [7:0] source = 8'h3c;
  logic enable = 0;
  logic eq_nba, inside_nba;
  logic [7:0] cast_nba, pattern_nba;
  logic [7:0] comb_nba, latch_nba;
  logic [7:0] const_ref_value;

  function automatic logic [7:0] const_ref_read(const ref logic [7:0] value);
    return value ^ 8'h80;
  endfunction

  always_comb begin
    eq_nba <= source == 8'h3c;
    inside_nba <= source inside {8'h3c};
    cast_nba <= byte_t'(source);
    pattern_nba <= '{source[7],source[6],source[5],source[4],source[3],source[2],source[1],source[0]};
    comb_nba <= source + 8'h03;
  end

  always_latch if (enable) begin
    latch_nba <= source + 8'h04;
  end

  assign const_ref_value = const_ref_read(source);

  initial begin
    #1;
    if (eq_nba !== 1 || inside_nba !== 1 || cast_nba !== 8'h3c ||
        pattern_nba !== 8'h3c || comb_nba !== 8'h3f)
      $fatal(1, "initial NBA values");

    enable = 1;
    source = 8'h5a;
    #1;
    if (eq_nba !== 0 || inside_nba !== 0 || cast_nba !== 8'h5a ||
        pattern_nba !== 8'h5a || comb_nba !== 8'h5d || latch_nba !== 8'h5e)
      $fatal(1, "first process update");
    if (const_ref_value !== 8'hda)
      $fatal(1, "first continuous call update");

    source = 8'h21;
    #1;
    if (eq_nba !== 0 || inside_nba !== 0 || cast_nba !== 8'h21 ||
        pattern_nba !== 8'h21 || comb_nba !== 8'h24 || latch_nba !== 8'h25)
      $fatal(1, "second process update");
    if (const_ref_value !== 8'ha1)
      $fatal(1, "second continuous call update");

    $display("ops=%b,%b,%h,%h comb=%h latch=%h const_ref=%h",
             eq_nba, inside_nba, cast_nba, pattern_nba, comb_nba,
             latch_nba, const_ref_value);
    $finish(0);
  end
endmodule
