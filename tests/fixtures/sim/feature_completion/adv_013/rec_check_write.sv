// ADV-013 A02: the 16.13.17 example `check_write` / `check_write_data_beat`
// (L25637-25686): two beats, a wrong second beat and a retry that restarts
// the beats. Expected lines are derived in readme.md.
`timescale 1ns/1ns
module tb;
  logic clk = 0, reset = 0;
  logic write_request = 0, write_request_ack = 0;
  logic [3:0] write_request_ack_tag = 0;
  logic [0:127] model_data = 0;
  logic data_valid = 0, last_data_valid = 0, retry = 0;
  logic [3:0] data_valid_tag = 0, retry_tag = 0;
  logic [7:0] data = 0;
  always #5 clk = ~clk;

  property check_write;
    logic [0:127] expected_data;
    logic [3:0] tag;
    disable iff (reset)
    (
      write_request && write_request_ack,
      expected_data = model_data,
      tag = write_request_ack_tag
    )
    |=>
    check_write_data_beat(expected_data, tag, 4'h0);
  endproperty

  property check_write_data_beat
  (
    local input logic [0:127] expected_data,
    local input logic [3:0] tag, i
  );
    (
      (data_valid && (data_valid_tag == tag))
      ||
      (retry && (retry_tag == tag))
    )[->1]
    |->
    (
      (
        (data_valid && (data_valid_tag == tag))
        |->
        (data == expected_data[i*8+:8])
      )
      and
      (
        if (retry && (retry_tag == tag))
        (
          1'b1 |=> check_write_data_beat(expected_data, tag, 4'h0)
        )
        else if (!last_data_valid)
        (
          1'b1 |=> check_write_data_beat(expected_data, tag, i+4'h1)
        )
        else
        (
          ##1 (retry && (retry_tag == tag))
          |=>
          check_write_data_beat(expected_data, tag, 4'h0)
        )
      )
    );
  endproperty

  cw: assert property (@(posedge clk) check_write)
        $display("cw P %0t", $time); else $display("cw F %0t", $time);

  task automatic request(input logic [3:0] tag, input logic [0:127] model);
    write_request = 1'b1; write_request_ack = 1'b1; write_request_ack_tag = tag; model_data = model;
    #10 write_request = 1'b0; write_request_ack = 1'b0;
  endtask
  task automatic beat(input logic [3:0] tag, input logic [7:0] value, input logic last);
    data_valid = 1'b1; data_valid_tag = tag; data = value; last_data_valid = last;
    #10 data_valid = 1'b0; last_data_valid = 1'b0;
  endtask
  task automatic retry_of(input logic [3:0] tag);
    retry = 1'b1; retry_tag = tag;
    #10 retry = 1'b0;
  endtask

  initial begin
    #12;
    request(4'd3, {8'hA1, 8'hA2, 112'h0});      // tick 15
    #10 beat(4'd3, 8'hA1, 0);                   // tick 35
    #10 beat(4'd3, 8'hA2, 1);                   // tick 55
    #10 request(4'd5, {8'hB1, 8'hB2, 8'hB3, 104'h0}); // tick 75
    #10 beat(4'd5, 8'hB1, 0);                   // tick 95
    beat(4'd5, 8'h00, 0);                       // tick 105 (wrong)
    #10 request(4'd7, {8'hC1, 8'hC2, 112'h0});  // tick 125
    #10 beat(4'd7, 8'hC1, 0);                   // tick 145
    retry_of(4'd7);                             // tick 155
    #10 beat(4'd7, 8'hC1, 0);                   // tick 175
    beat(4'd7, 8'hC2, 1);                       // tick 185
    #30 $finish;
  end
endmodule
