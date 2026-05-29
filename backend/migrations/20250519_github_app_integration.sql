-- GitHub App Integration Migration
-- Created: 2025-05-19

-- Fix github_installations table to match schema expectations
ALTER TABLE github_installations DROP COLUMN IF EXISTS updated_at;
ALTER TABLE github_installations ADD COLUMN IF NOT EXISTS updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW();

-- Add missing columns to github_installations
ALTER TABLE github_installations ADD COLUMN IF NOT EXISTS repository_selection TEXT DEFAULT 'all';

-- Create build_status table for tracking builds
CREATE TABLE IF NOT EXISTS build_status (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    conclusion TEXT,
    build_logs TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create build_logs table for build step outputs
CREATE TABLE IF NOT EXISTS build_logs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    build_id UUID REFERENCES build_status(id),
    step TEXT NOT NULL,
    output TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create deployments table for tracking deployments
CREATE TABLE IF NOT EXISTS deployments (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository TEXT NOT NULL,
    commit_sha TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    deployed_at TIMESTAMP WITH TIME ZONE,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create cicd_pipelines table for tracking CI/CD pipelines
CREATE TABLE IF NOT EXISTS cicd_pipelines (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    commit_sha TEXT,
    pr_number INTEGER,
    triggered_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Create pull_request_tests table for tracking PR test runs
CREATE TABLE IF NOT EXISTS pull_request_tests (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    repository TEXT NOT NULL,
    pr_number INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    conclusion TEXT,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
    completed_at TIMESTAMP WITH TIME ZONE
);

-- Create indexes for build_status
CREATE INDEX IF NOT EXISTS idx_build_status_repository ON build_status(repository);
CREATE INDEX IF NOT EXISTS idx_build_status_commit_sha ON build_status(commit_sha);
CREATE INDEX IF NOT EXISTS idx_build_status_status ON build_status(status);

-- Create indexes for build_logs
CREATE INDEX IF NOT EXISTS idx_build_logs_build_id ON build_logs(build_id);

-- Create indexes for deployments
CREATE INDEX IF NOT EXISTS idx_deployments_repository ON deployments(repository);
CREATE INDEX IF NOT EXISTS idx_deployments_status ON deployments(status);

-- Create indexes for cicd_pipelines
CREATE INDEX IF NOT EXISTS idx_cicd_pipelines_repository ON cicd_pipelines(repository);
CREATE INDEX IF NOT EXISTS idx_cicd_pipelines_status ON cicd_pipelines(status);

-- Create indexes for pull_request_tests
CREATE INDEX IF NOT EXISTS idx_pull_request_tests_repository ON pull_request_tests(repository);
CREATE INDEX IF NOT EXISTS idx_pull_request_tests_pr_number ON pull_request_tests(pr_number);
CREATE INDEX IF NOT EXISTS idx_pull_request_tests_status ON pull_request_tests(status);
