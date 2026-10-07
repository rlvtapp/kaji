package main

import (
	"context"
	"fmt"
	"io"
	"net"
	"net/http"
	"net/netip"
	"net/url"
	"path/filepath"
	"strings"
	"time"
)

func isRemoteDocument(path string) bool {
	return strings.HasPrefix(path, "https://") || strings.HasPrefix(path, "http://")
}

func resolveSourceReference(origin, value string) (string, string, error) {
	ref, err := url.Parse(value)
	if err != nil {
		return "", "", err
	}
	fragment := ref.Fragment
	ref.Fragment = ""
	if ref.User != nil {
		return "", "", fmt.Errorf("reference URLs must not contain credentials")
	}
	if isRemoteDocument(origin) {
		if ref.Path == "" && ref.Scheme == "" && ref.Host == "" && ref.RawQuery == "" {
			return origin, fragment, nil
		}
		base, _ := url.Parse(origin)
		ref = base.ResolveReference(ref)
	}
	if ref.Scheme != "" || ref.Host != "" {
		if ref.Scheme == "http" && ref.String() == origin {
			return origin, fragment, nil
		}
		if ref.Scheme != "https" || ref.Host == "" {
			return "", "", fmt.Errorf("remote references require HTTPS")
		}
		return ref.String(), fragment, nil
	}
	if ref.RawQuery != "" {
		return "", "", fmt.Errorf("local references cannot contain a query")
	}
	path := origin
	if ref.Path != "" {
		path = ref.Path
		if !filepath.IsAbs(path) {
			path = filepath.Join(filepath.Dir(origin), filepath.FromSlash(path))
		}
	}
	return filepath.Clean(path), fragment, nil
}

// Resolve and validate every address before dialing the chosen address directly:
// DNS rebinding cannot turn an approved public hostname into an internal request.
func publicReferenceDial(ctx context.Context, network, address string) (net.Conn, error) {
	host, port, err := net.SplitHostPort(address)
	if err != nil {
		return nil, err
	}
	addresses, err := net.DefaultResolver.LookupNetIP(ctx, "ip", host)
	if err != nil {
		return nil, err
	}
	if len(addresses) == 0 {
		return nil, fmt.Errorf("reference host has no addresses")
	}
	for _, ip := range addresses {
		ip = ip.Unmap()
		if !publicReferenceAddress(ip) {
			return nil, fmt.Errorf("remote references cannot access non-public addresses")
		}
	}
	return (&net.Dialer{Timeout: 10 * time.Second}).DialContext(ctx, network, net.JoinHostPort(addresses[0].String(), port))
}

func fetchRemoteSource(target string) ([]byte, error) {
	uri, err := url.Parse(target)
	if err != nil || uri.Scheme != "https" || uri.User != nil || uri.Host == "" {
		return nil, fmt.Errorf("invalid HTTPS reference URL")
	}
	transport := &http.Transport{DialContext: publicReferenceDial, TLSHandshakeTimeout: 10 * time.Second, ResponseHeaderTimeout: 10 * time.Second}
	defer transport.CloseIdleConnections()
	client := &http.Client{Transport: transport, Timeout: 30 * time.Second, CheckRedirect: func(req *http.Request, via []*http.Request) error {
		return fmt.Errorf("remote reference redirects are disabled; use the final HTTPS URL")
	}}
	return fetchRemoteSourceWithClient(target, client)
}

func fetchRemoteSourceWithClient(target string, client *http.Client) ([]byte, error) {
	req, err := http.NewRequest(http.MethodGet, target, nil)
	if err != nil {
		return nil, err
	}
	req.Header.Set("Accept", "application/json, application/yaml, text/yaml, */*")
	response, err := client.Do(req)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()
	if response.StatusCode != http.StatusOK {
		return nil, fmt.Errorf("remote reference returned HTTP %d", response.StatusCode)
	}
	data, err := io.ReadAll(io.LimitReader(response.Body, (16<<20)+1))
	if err != nil {
		return nil, err
	}
	if len(data) > 16<<20 {
		return nil, fmt.Errorf("remote reference exceeds 16 MiB")
	}
	return data, nil
}

// Exclude non-public special-purpose networks even when IsGlobalUnicast is true.
func publicReferenceAddress(ip netip.Addr) bool {
	ip = ip.Unmap()
	if !ip.IsGlobalUnicast() || ip.IsPrivate() || ip.IsLoopback() || ip.IsLinkLocalUnicast() || ip.IsUnspecified() {
		return false
	}
	for _, block := range []string{"0.0.0.0/8", "100.64.0.0/10", "192.0.0.0/24", "192.0.2.0/24", "192.88.99.0/24", "198.18.0.0/15", "198.51.100.0/24", "203.0.113.0/24", "240.0.0.0/4", "64:ff9b::/96", "64:ff9b:1::/48", "100::/64", "2001::/23", "2002::/16"} {
		if netip.MustParsePrefix(block).Contains(ip) {
			return false
		}
	}
	return true
}
