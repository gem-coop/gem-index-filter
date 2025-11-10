# Webhook Server

A high-performance webhook server that automatically filters the RubyGems versions index and uploads the filtered results to S3.

## Features

- **Webhook-triggered processing**: Accepts POST requests to `/webhook` endpoint
- **Async processing**: Returns immediately (202 Accepted) while processing in background
- **Streaming filtering**: Uses gem-index-filter library for memory-efficient processing
- **S3 integration**: Fetches allowlist from S3 and uploads filtered results
- **SHA-256 checksums**: Automatically computes and stores checksums
- **Latest pointers**: Updates `filtered-latest.bin` and `filtered-latest.sha256` for easy access
- **Graceful shutdown**: Waits for active tasks to complete on shutdown

## Building

Build the webhook server with the `server` feature:

```bash
cargo build --bin webhook-server --features server --release
```

## Configuration

Configure via environment variables:

| Variable | Description | Default |
|----------|-------------|---------|
| `BUCKET_NAME` | S3 bucket for allowlist and output | `rubygems-filtered` |
| `ALLOWLIST_KEY` | S3 key for allowlist file | `allowlist.txt` |

AWS credentials are loaded from the environment (via AWS SDK defaults).

## Running

```bash
export BUCKET_NAME=my-rubygems-bucket
export ALLOWLIST_KEY=allowlist.txt
export AWS_REGION=us-east-1
export AWS_ACCESS_KEY_ID=...
export AWS_SECRET_ACCESS_KEY=...

./target/release/webhook-server
```

The server listens on `0.0.0.0:8080`.

## API

### POST /webhook

Triggers index filtering and S3 upload.

**Request:**
```bash
curl -X POST http://localhost:8080/webhook
```

**Response:**
```json
{
  "status": "accepted"
}
```

HTTP 202 Accepted - Processing happens in background.

## Processing Flow

1. **Webhook received**: Returns 202 immediately, spawns background task
2. **Fetch allowlist**: Downloads allowlist from S3 (`BUCKET_NAME/ALLOWLIST_KEY`)
3. **Download index**: Fetches https://index.rubygems.org/versions
4. **Filter**: Uses gem-index-filter to filter with allowlist, strips versions
5. **Compute checksum**: Calculates SHA-256 of filtered output
6. **Upload timestamped files**:
   - `versions/filtered-YYYYMMDD-HHMMSS.bin`
   - `versions/filtered-YYYYMMDD-HHMMSS.sha256`
7. **Update latest pointers**:
   - `versions/filtered-latest.bin` → latest timestamped version
   - `versions/filtered-latest.sha256` → latest checksum

## Allowlist Format

The allowlist file in S3 should contain one gem name per line:

```
rails
sinatra
puma
rack
# Comments are supported
nokogiri
```

Empty lines and lines starting with `#` are ignored.

## Example S3 Structure

After processing, your S3 bucket will contain:

```
s3://my-rubygems-bucket/
  allowlist.txt
  versions/
    filtered-20241110-143052.bin
    filtered-20241110-143052.sha256
    filtered-20241110-154521.bin
    filtered-20241110-154521.sha256
    filtered-latest.bin          -> copy of most recent
    filtered-latest.sha256       -> copy of most recent checksum
```

## Performance

- **Input**: ~21MB RubyGems versions index
- **Allowlist**: 10,000 gems
- **Processing time**: ~100ms filtering + network I/O
- **Memory**: ~3MB for filtering
- **Output**: Typically <1MB (with version stripping)

## Graceful Shutdown

Press Ctrl+C to initiate graceful shutdown. The server will:

1. Stop accepting new webhook requests
2. Wait for all active background tasks to complete
3. Exit cleanly

## Integration Examples

### With RubyGems Webhook

Configure RubyGems.org to send webhook notifications to your server when gems are updated.

### Scheduled Updates (cron)

```bash
# Update every hour
0 * * * * curl -X POST https://your-server.com/webhook
```

### GitHub Actions

```yaml
- name: Trigger gem index update
  run: curl -X POST https://your-server.com/webhook
```

## Monitoring

The server logs to stdout/stderr:

```
Listening on 0.0.0.0:8080
Fetching allowlist from S3: rubygems-filtered/allowlist.txt
Loaded 10000 gems in allowlist
Fetching RubyGems index from https://index.rubygems.org/versions
Downloaded 21428463 bytes, filtering...
Filtered to 823451 bytes, SHA-256: abc123...
Uploaded: versions/filtered-20241110-143052.bin and versions/filtered-20241110-143052.sha256 (also updated latest pointers)
```

## Error Handling

- **S3 errors**: Logged to stderr, task fails but server continues
- **Network errors**: Logged to stderr, task fails but server continues
- **Filter errors**: Logged to stderr, task fails but server continues

All errors are non-fatal to the server process - only the individual webhook task fails.

## Security Considerations

- **No authentication**: Add a reverse proxy (nginx, API Gateway) for authentication
- **Rate limiting**: Not implemented - add via reverse proxy if needed
- **IAM permissions**: Server needs S3 read/write access to configured bucket

Required IAM permissions:

```json
{
  "Version": "2012-10-17",
  "Statement": [
    {
      "Effect": "Allow",
      "Action": [
        "s3:GetObject",
        "s3:PutObject",
        "s3:CopyObject"
      ],
      "Resource": "arn:aws:s3:::my-rubygems-bucket/*"
    }
  ]
}
```
