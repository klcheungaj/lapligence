// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/typed_continuous_matrix.sv
// IEEE 1800-2009 §§6.5, 10.3, 10.4, and 10.6: typed continuous
// assignments exercise whole drivers, selected targets, and self-source projections.

module tb;
  typedef enum logic [7:0] {IDLE=8'h00, ACTIVE=8'h01} state_t;
  typedef struct packed {logic [7:0] hi; logic [7:0] lo;} pair_t;
  typedef union packed {logic [15:0] word; pair_t halves;} union_t;
  typedef struct {logic [7:0] key; logic [7:0] payload;} record_t;
  typedef record_t records_t [0:1];
  logic [7:0] source = 8'h01;
  pair_t pair_source = '{hi:8'h12,lo:8'h34};
  union_t union_source = union_t'(16'ha5c3);
  record_t record_source = '{key:8'h56,payload:8'h78};
  records_t records_source = '{0:'{key:8'h11,payload:8'h22},1:'{key:8'h33,payload:8'h44}};
  wire [7:0] integral_net;
  wire state_t enum_net;
  state_t enum_var;
  wire pair_t pair_net;
  pair_t pair_var;
  wire union_t union_net;
  union_t union_var;
  wire record_t record_net;
  record_t record_var;
  wire records_t records_net;
  records_t records_var;
  assign integral_net = source;
  assign enum_net = state_t'(source);
  assign enum_var = state_t'(source);
  assign pair_net = pair_source;
  assign pair_var = pair_source;
  assign union_net = union_source;
  assign union_var = union_source;
  assign record_net = record_source;
  assign record_var = record_source;
  assign records_net = records_source;
  assign records_var = records_source;
  wire pair_t field_net;
  pair_t field_var;
  wire [7:0] concat_net;
  logic [7:0] concat_var;
  logic [7:0] slice_var;
  assign field_net.hi = pair_source.hi;
  assign field_net.lo = pair_source.lo;
  assign field_var.hi = pair_source.hi;
  assign field_var.lo = pair_source.lo;
  assign {concat_net[7:4], concat_net[3:0]} = 8'hb2;
  assign {concat_var[7:4], concat_var[3:0]} = 8'hb3;
  assign slice_var[7:4] = 4'hc;
  assign slice_var[3:0] = 4'h6;
  logic select = 1'b1;
  logic [7:0] conditional_var;
  wire [7:0] equality_net;
  logic [7:0] equality_var;
  wire [7:0] cast_net;
  logic [7:0] cast_var;
  wire [7:0] pattern_net;
  logic [7:0] pattern_var;
  assign conditional_var[7:1] = 7'b0000001;
  assign conditional_var[0] = select ? conditional_var[1] : conditional_var[2];
  assign equality_net[7:1] = 7'b0000001;
  assign equality_net[0] = equality_net[1] == 1'b1;
  assign equality_var[7:1] = 7'b0000001;
  assign equality_var[0] = equality_var[1] == 1'b1;
  assign cast_net[7:1] = 7'b0000001;
  assign cast_net[0] = bit'(cast_net[1]);
  assign cast_var[7:1] = 7'b0000001;
  assign cast_var[0] = bit'(cast_var[1]);
  assign pattern_net[7:2] = 6'b000001;
  assign pattern_net[1:0] = '{pattern_net[3], pattern_net[2]};
  assign pattern_var[7:2] = 6'b000001;
  assign pattern_var[1:0] = '{pattern_var[3], pattern_var[2]};
  initial begin
    #1;
    if (integral_net !== 8'h01 || enum_net !== ACTIVE || enum_var !== ACTIVE ||
        pair_net !== pair_source || pair_var !== pair_source ||
        union_net.word !== 16'ha5c3 || union_var.word !== 16'ha5c3 ||
        record_net.key !== 8'h56 || record_net.payload !== 8'h78 ||
        record_var.key !== 8'h56 || record_var.payload !== 8'h78 ||
        records_net[0].key !== 8'h11 || records_net[0].payload !== 8'h22 ||
        records_net[1].key !== 8'h33 || records_net[1].payload !== 8'h44 ||
        records_var[0].key !== 8'h11 || records_var[0].payload !== 8'h22 ||
        records_var[1].key !== 8'h33 || records_var[1].payload !== 8'h44)
        $fatal(1,"typed continuous mismatch");
    if (field_net !== pair_source || field_var !== pair_source || concat_net !== 8'hb2 || concat_var !== 8'hb3 || slice_var !== 8'hc6) $fatal(1,"lvalue continuous mismatch");
    if (conditional_var !== 8'h03 || equality_net !== 8'h03 || equality_var !== 8'h03 || cast_net !== 8'h03 || cast_var !== 8'h03 || pattern_net !== 8'h05 || pattern_var !== 8'h05) $fatal(1,"operation continuous mismatch");
    $display("operation=%h,%h,%h,%h,%h,%h,%h",conditional_var,equality_net,equality_var,cast_net,cast_var,pattern_net,pattern_var);
    $display("lvalue=%h,%h,%h,%h,%h",field_net,field_var,concat_net,concat_var,slice_var);
    $display("cont=%h,%h,%h,%h,%h,%h,%h,%h,%h,%h,%h",integral_net,enum_net,enum_var,pair_net,pair_var,union_net.word,union_var.word,record_net.key,record_var.payload,records_net[0].key,records_var[1].payload);
    $finish(0);
  end
endmodule
