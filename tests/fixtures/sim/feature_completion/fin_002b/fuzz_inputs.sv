// FIN-002 step 3c: one adversarial input file `in.mem` and the plusargs of
// the run go through every run-time reader of text and binary input:
// $readmemh/$readmemb (whole memory, address range, descending and wide
// memories), $fscanf, $fgets, $fgetc, $fread, $sscanf and $value$plusargs.
// The suite runs the built model once per generated input. Text read from
// the file is printed as lengths and sums only, so the transcript stays
// plain ASCII whatever the input holds.
module tb;
  logic [15:0] words [0:15];
  bit   [7:0]  bytes [3:10];
  logic [99:0] wide  [7:0];
  logic [31:0] image [0:3];
  logic [39:0] packed_text;
  logic [63:0] vector;
  integer fd, code, number, i, sum;
  int     count;
  real    fraction;
  string  text;

  function automatic integer checksum(string s);
    checksum = 0;
    for (int k = 0; k < s.len(); k++) checksum = (checksum * 31 + s[k]) % 65521;
  endfunction

  initial begin
    $readmemh("in.mem", words);
    for (i = 0; i < 16; i++) $display("h %0d %h", i, words[i]);
    $readmemb("in.mem", bytes, 4, 9);
    for (i = 3; i <= 10; i++) $display("b %0d %b", i, bytes[i]);
    $readmemh("in.mem", wide);
    for (i = 7; i >= 0; i--) $display("w %0d %h", i, wide[i]);

    fd = $fopen("in.mem", "r");
    if (fd == 0) begin
      $display("no input");
    end else begin
      number = 0; vector = '0; text = "";
      code = $fscanf(fd, "%d %h %s", number, vector, text);
      $display("scan1 %0d %0d %h %0d %0d", code, number, vector, text.len(), checksum(text));
      code = $fgets(text, fd);
      $display("gets %0d %0d %0d", code, text.len(), checksum(text));
      code = $fgetc(fd);
      $display("getc %0d", code);
      vector = '0; fraction = 0.0; number = 0;
      code = $fscanf(fd, " %b %o %f %c", vector, number, fraction, count);
      $display("scan2 %0d %h %0d %0d", code, vector, number, count);
      packed_text = '0;
      code = $fgets(packed_text, fd);
      $display("getp %0d %h", code, packed_text);
      code = $fread(image, fd);
      $display("fread %0d %h %h %h %h", code, image[0], image[1], image[2], image[3]);
      code = $fread(vector, fd);
      $display("freadv %0d %h", code, vector);
      code = $ungetc(65, fd);
      code = $fgetc(fd);
      $display("unget %0d eof %0d", code, $feof(fd) != 0);
      $fclose(fd);
      code = $fgetc(fd);
      $display("closed %0d", code);
    end

    if ($value$plusargs("text=%s", text)) begin
      number = 0; vector = '0;
      code = $sscanf(text, "%d%h", number, vector);
      $display("sscanf %0d %0d %h %0d", code, number, vector, text.len());
    end
    number = -1;
    if ($value$plusargs("dec=%d", number)) $display("dec %0d", number);
    vector = '1;
    if ($value$plusargs("hex=%h", vector)) $display("hex %h", vector);
    if ($value$plusargs("bin=%b", vector)) $display("bin %h", vector);
    if ($value$plusargs("oct=%o", vector)) $display("oct %h", vector);
    fraction = -1.0;
    if ($value$plusargs("real=%f", fraction)) $display("real %0d", fraction == fraction);
    $display("flag %0d %0d", $test$plusargs("fl"), $test$plusargs(""));
    $display("done");
    $finish;
  end
endmodule
