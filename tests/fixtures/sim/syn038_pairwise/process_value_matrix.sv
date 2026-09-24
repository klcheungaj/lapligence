// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/process_value_matrix.sv
// IEEE 1800-2009 §§9.2, 9.4, 10.9, and 10.10: typed lvalues and operation
// results are checked through blocking, combinational, latch, and NBA processes.
package types;
  typedef logic [7:0] byte_t;
  typedef enum logic [7:0] {IDLE=8'h00, ACTIVE=8'h01} state_t;
  typedef struct packed {logic [7:0] hi; logic [7:0] lo;} pair_t;
  typedef union packed {logic [15:0] word; pair_t parts;} union_t;
  typedef struct {logic [7:0] key; logic [7:0] payload;} record_t;
  typedef record_t records_t [0:1];
  typedef logic [7:0] bytes_t [0:1];
endpackage
module tb;
  import types::*;
  logic trigger = 0;
  logic latch_open = 0;
  logic clk = 0;
  logic select = 1;
  byte_t source = 8'ha5;
  logic [6:0] cast_source = 7'h25;
  record_t source_record = '{key:8'h12,payload:8'h34};
  records_t source_records = '{0:'{key:8'h11,payload:8'h22},1:'{key:8'h33,payload:8'h44}};
  bytes_t source_bytes = '{8'h55,8'h66};
  pair_t always_pair;
  union_t always_union;
  bytes_t always_array;
  byte_t always_slice;
  byte_t always_concat;
  byte_t always_pattern;
  byte_t comb_concat;
  byte_t comb_pattern;
  record_t comb_record;
  records_t comb_records;
  byte_t comb_conditional;
  logic comb_equal;
  byte_t comb_cast;
  byte_t latch_concat;
  byte_t latch_pattern;
  state_t latch_enum;
  bytes_t latch_array;
  logic latch_equal;
  byte_t latch_cast;
  byte_t latch_pattern_value;
  byte_t ff_slice;
  byte_t ff_pattern;
  record_t ff_record;
  logic ff_equal;
  always @(trigger) begin
    if (trigger) begin
      always_pair.hi = 8'h12;
      always_union.word = 16'habcd;
      always_array[1] = 8'h55;
      always_slice[7:4] = 4'ha;
      {always_concat[7:4],always_concat[3:0]} = 8'hb2;
      '{always_pattern[7],always_pattern[0]} = 2'b01;
    end
  end
  always_comb begin
    {comb_concat[7:4],comb_concat[3:0]} = 8'hc3;
    '{comb_pattern[7],comb_pattern[0]} = 2'b01;
    comb_record = source_record;
    comb_records = source_records;
    comb_conditional = select ? source : 8'h11;
    comb_equal = source == 8'ha5;
    comb_cast = byte_t'(cast_source);
  end
  always_latch begin
    if (latch_open) begin
      {latch_concat[7:4],latch_concat[3:0]} = 8'hd4;
      '{latch_pattern[7],latch_pattern[0]} = 2'b01;
      latch_enum = ACTIVE;
      latch_array = source_bytes;
      latch_equal = source == 8'ha5;
      latch_cast = byte_t'(cast_source);
      latch_pattern_value = '{source[7],source[6],source[5],source[4],source[3],source[2],source[1],source[0]};
    end
  end
  always_ff @(posedge clk) begin
    ff_slice[7:4] <= 4'he;
    '{ff_pattern[7],ff_pattern[0]} <= 2'b01;
    ff_record <= source_record;
    ff_equal <= source == 8'ha5;
  end
  initial begin
    #1;
    if (comb_concat !== 8'hc3 || comb_pattern[7] !== 0 || comb_pattern[0] !== 1 || comb_record.key !== 8'h12 || comb_record.payload !== 8'h34 || comb_records[0].key !== 8'h11 || comb_records[0].payload !== 8'h22 || comb_records[1].key !== 8'h33 || comb_records[1].payload !== 8'h44 || comb_conditional !== 8'ha5 || comb_equal !== 1 || comb_cast !== 8'h25) $fatal(1,"comb");
    latch_open = 1;
    trigger = 1;
    #1;
    if (always_pair.hi !== 8'h12 || always_union.word !== 16'habcd || always_array[1] !== 8'h55 || always_slice[7:4] !== 4'ha || always_concat !== 8'hb2 || always_pattern[7] !== 0 || always_pattern[0] !== 1) $fatal(1,"always");
    if (latch_concat !== 8'hd4 || latch_pattern[7] !== 0 || latch_pattern[0] !== 1 || latch_enum !== ACTIVE || latch_array[0] !== 8'h55 || latch_array[1] !== 8'h66 || latch_equal !== 1 || latch_cast !== 8'h25 || latch_pattern_value !== 8'ha5) $fatal(1,"latch");
    latch_open = 0;
    clk = 1;
    #1;
    if (ff_slice[7:4] !== 4'he || ff_pattern[7] !== 0 || ff_pattern[0] !== 1 || ff_record.key !== 8'h12 || ff_record.payload !== 8'h34 || ff_equal !== 1) $fatal(1,"ff");
    $display("process=%h,%h,%h,%h,%h,%b %h,%b,%h/%h,%h/%h/%h/%h,%h,%b,%h %h,%b,%h,%h/%h,%b,%h,%h %h,%b,%h/%h,%b",
             always_pair.hi, always_union.word, always_array[1], always_slice[7:4],
             always_concat, {always_pattern[7],always_pattern[0]},
             comb_concat, {comb_pattern[7],comb_pattern[0]},
             comb_record.key, comb_record.payload,
             comb_records[0].key, comb_records[0].payload,
             comb_records[1].key, comb_records[1].payload,
             comb_conditional, comb_equal, comb_cast,
             latch_concat, {latch_pattern[7],latch_pattern[0]}, latch_enum,
             latch_array[0], latch_array[1], latch_equal, latch_cast, latch_pattern_value,
             ff_slice[7:4], {ff_pattern[7],ff_pattern[0]}, ff_record.key, ff_record.payload, ff_equal);
    $finish(0);
  end
endmodule
